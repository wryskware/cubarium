//! The UDP wire format: a 16-byte little-endian header followed by the payload.
//!
//! | offset | size | field    | notes                                                  |
//! |--------|------|----------|--------------------------------------------------------|
//! | 0      | 4    | magic    | `b"CUBE"`                                              |
//! | 4      | 1    | version  | `1`                                                    |
//! | 5      | 1    | format   | `0` = full frame, `1` = single face                    |
//! | 6      | 1    | face     | face index for format 1, `0xFF` for format 0           |
//! | 7      | 1    | flags    | reserved, must be 0                                    |
//! | 8      | 4    | seq      | u32, per-sender, monotonically increasing              |
//! | 12     | 4    | reserved | 0                                                      |

use crate::{Face, Frame, FACE_BYTES, FRAME_BYTES};

pub const HEADER_BYTES: usize = 16;
pub const MAGIC: &[u8; 4] = b"CUBE";
pub const VERSION: u8 = 1;
/// The `face` byte for a full-frame datagram.
pub const NO_FACE: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    FullFrame,
    SingleFace,
}

impl Format {
    #[inline]
    pub fn code(self) -> u8 {
        match self {
            Format::FullFrame => 0,
            Format::SingleFace => 1,
        }
    }

    /// Exact payload length a datagram of this format must carry.
    #[inline]
    pub fn payload_len(self) -> usize {
        match self {
            Format::FullFrame => FRAME_BYTES,
            Format::SingleFace => FACE_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub format: Format,
    pub face: Option<Face>,
    pub seq: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// Fewer than `HEADER_BYTES` bytes received.
    TooShort { len: usize },
    BadMagic([u8; 4]),
    BadVersion(u8),
    BadFormat(u8),
    /// Face byte is neither a valid index (format 1) nor `0xFF` (format 0).
    BadFace(u8),
    BadFlags(u8),
    BadPayloadLen { expected: usize, got: usize },
}

impl std::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoError::TooShort { len } => {
                write!(f, "datagram too short: {len} bytes, need at least {HEADER_BYTES}")
            }
            ProtoError::BadMagic(m) => write!(f, "bad magic {m:02x?}, expected {MAGIC:02x?}"),
            ProtoError::BadVersion(v) => write!(f, "unsupported version {v}, expected {VERSION}"),
            ProtoError::BadFormat(v) => write!(f, "unknown format {v}"),
            ProtoError::BadFace(v) => write!(f, "bad face byte {v:#04x}"),
            ProtoError::BadFlags(v) => write!(f, "flags must be 0, got {v:#04x}"),
            ProtoError::BadPayloadLen { expected, got } => {
                write!(f, "payload length {got}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ProtoError {}

fn write_header(out: &mut Vec<u8>, format: Format, face: u8, seq: u32) {
    out.clear();
    out.reserve(HEADER_BYTES + format.payload_len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(format.code());
    out.push(face);
    out.push(0); // flags
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
}

/// Encode a full-frame datagram into `out` (cleared first). 61,456 bytes total.
pub fn encode_full(frame: &Frame, seq: u32, out: &mut Vec<u8>) {
    write_header(out, Format::FullFrame, NO_FACE, seq);
    out.extend_from_slice(frame.as_bytes());
}

/// Encode a single-face datagram into `out` (cleared first). 12,304 bytes total.
pub fn encode_face(frame: &Frame, face: Face, seq: u32, out: &mut Vec<u8>) {
    write_header(out, Format::SingleFace, face.index() as u8, seq);
    out.extend_from_slice(frame.face(face));
}

/// Validate a datagram and split it into header and payload.
///
/// The trailing 4 reserved header bytes are ignored on receive (forward compatibility);
/// everything else in the header is checked.
pub fn decode(datagram: &[u8]) -> Result<(Header, &[u8]), ProtoError> {
    if datagram.len() < HEADER_BYTES {
        return Err(ProtoError::TooShort {
            len: datagram.len(),
        });
    }
    let magic: [u8; 4] = datagram[0..4].try_into().unwrap();
    if &magic != MAGIC {
        return Err(ProtoError::BadMagic(magic));
    }
    if datagram[4] != VERSION {
        return Err(ProtoError::BadVersion(datagram[4]));
    }
    let format = match datagram[5] {
        0 => Format::FullFrame,
        1 => Format::SingleFace,
        v => return Err(ProtoError::BadFormat(v)),
    };
    let face_byte = datagram[6];
    if datagram[7] != 0 {
        return Err(ProtoError::BadFlags(datagram[7]));
    }
    let seq = u32::from_le_bytes(datagram[8..12].try_into().unwrap());

    let face = match format {
        Format::FullFrame => {
            if face_byte != NO_FACE {
                return Err(ProtoError::BadFace(face_byte));
            }
            None
        }
        Format::SingleFace => Some(Face::from_index(face_byte).ok_or(ProtoError::BadFace(face_byte))?),
    };

    let payload = &datagram[HEADER_BYTES..];
    let expected = format.payload_len();
    if payload.len() != expected {
        return Err(ProtoError::BadPayloadLen {
            expected,
            got: payload.len(),
        });
    }
    Ok((Header { format, face, seq }, payload))
}

/// Wraparound-tolerant sequence comparison: is `new` newer than `last`?
///
/// Sequence numbers are u32 and wrap; anything within half the u32 range ahead of `last`
/// counts as newer. Equal sequence numbers are not newer (the receiver drops seq ≤ last).
#[inline]
pub fn seq_is_newer(new: u32, last: u32) -> bool {
    (new.wrapping_sub(last) as i32) > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_frame_header_bytes_for_seq_1() {
        let mut buf = Vec::new();
        encode_full(&Frame::black(), 1, &mut buf);
        assert_eq!(buf.len(), HEADER_BYTES + FRAME_BYTES);
        assert_eq!(
            &buf[..HEADER_BYTES],
            &[
                0x43, 0x55, 0x42, 0x45, 0x01, 0x00, 0xff, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00
            ]
        );
    }

    #[test]
    fn round_trip_full_and_face() {
        let mut frame = Frame::black();
        frame.set(Face::Back, 3, 4, [1, 2, 3]);
        frame.set(Face::Top, 0, 0, [10, 20, 30]);

        let mut buf = Vec::new();
        encode_full(&frame, 7, &mut buf);
        let (h, payload) = decode(&buf).unwrap();
        assert_eq!(
            h,
            Header {
                format: Format::FullFrame,
                face: None,
                seq: 7
            }
        );
        assert_eq!(payload, frame.as_bytes().as_slice());

        encode_face(&frame, Face::Top, 8, &mut buf);
        let (h, payload) = decode(&buf).unwrap();
        assert_eq!(
            h,
            Header {
                format: Format::SingleFace,
                face: Some(Face::Top),
                seq: 8
            }
        );
        assert_eq!(payload, frame.face(Face::Top));
        assert_eq!(&payload[..3], &[10, 20, 30]);
    }

    #[test]
    fn decode_rejects_bad_input() {
        let mut good = Vec::new();
        encode_full(&Frame::black(), 1, &mut good);

        assert_eq!(decode(&good[..8]), Err(ProtoError::TooShort { len: 8 }));

        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(matches!(decode(&bad), Err(ProtoError::BadMagic(_))));

        let mut bad = good.clone();
        bad[4] = 2;
        assert_eq!(decode(&bad), Err(ProtoError::BadVersion(2)));

        let mut bad = good.clone();
        bad[5] = 9;
        assert_eq!(decode(&bad), Err(ProtoError::BadFormat(9)));

        let mut bad = good.clone();
        bad[6] = 0; // full frame must carry 0xFF
        assert_eq!(decode(&bad), Err(ProtoError::BadFace(0)));

        let mut bad = good.clone();
        bad[7] = 1;
        assert_eq!(decode(&bad), Err(ProtoError::BadFlags(1)));

        // Truncated payload.
        assert_eq!(
            decode(&good[..good.len() - 1]),
            Err(ProtoError::BadPayloadLen {
                expected: FRAME_BYTES,
                got: FRAME_BYTES - 1
            })
        );

        // Single-face datagram with an out-of-range face index.
        let mut face_dg = Vec::new();
        encode_face(&Frame::black(), Face::Front, 1, &mut face_dg);
        face_dg[6] = 5;
        assert_eq!(decode(&face_dg), Err(ProtoError::BadFace(5)));
    }

    #[test]
    fn seq_wraparound() {
        assert!(seq_is_newer(2, 1));
        assert!(!seq_is_newer(1, 1));
        assert!(!seq_is_newer(1, 2));
        assert!(seq_is_newer(0, u32::MAX));
        assert!(!seq_is_newer(u32::MAX, 0));
    }
}
