//! The UDP wire format: a 16-byte little-endian header followed by the payload.
//!
//! | offset | size | field    | notes                                                  |
//! |--------|------|----------|--------------------------------------------------------|
//! | 0      | 4    | magic    | `b"CUBE"`                                              |
//! | 4      | 1    | version  | `1`                                                    |
//! | 5      | 1    | format   | `0` = full frame, `1` = single face, `2` = raster strip |
//! | 6      | 1    | face     | face index for format 1, `0xFF` for formats 0 and 2    |
//! | 7      | 1    | flags    | reserved, must be 0                                    |
//! | 8      | 4    | seq      | u32, per-sender, monotonically increasing              |
//! | 12     | 4    | reserved | 0                                                      |

//!
//! Format 2's payload starts with its own 8-byte little-endian strip header —
//! `width u16, height u16, y0 u16, rows u16` — followed by `rows · width · 3` RGB8
//! bytes. See [`Strip`] and [`encode_raster`]. Formats 0 and 1 are byte-for-byte what
//! they have always been; format 2 is purely additive and the version stays 1.

use crate::raster::{dims_ok, Raster, MAX_RASTER_DIM};
use crate::{Face, Frame, FACE_BYTES, FRAME_BYTES};

pub const HEADER_BYTES: usize = 16;
pub const MAGIC: &[u8; 4] = b"CUBE";
pub const VERSION: u8 = 1;
/// The `face` byte for a full-frame datagram.
pub const NO_FACE: u8 = 0xFF;

/// Size of a raster strip's own header, in bytes, ahead of its pixels.
pub const STRIP_HEADER_BYTES: usize = 8;

/// The largest UDP payload that fits in one IPv4 datagram: 65,535 less the 20-byte IP
/// and 8-byte UDP headers. The default cut-off for [`encode_raster`].
pub const MAX_DATAGRAM: usize = 65_507;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    FullFrame,
    SingleFace,
    /// A horizontal band of a flat RGB8 image. Variable length.
    RasterStrip,
}

impl Format {
    #[inline]
    pub fn code(self) -> u8 {
        match self {
            Format::FullFrame => 0,
            Format::SingleFace => 1,
            Format::RasterStrip => 2,
        }
    }

    /// The payload length a datagram of this format must carry — exact for the two
    /// cube-frame formats, and for [`Format::RasterStrip`] the *minimum*, since its
    /// real length is whatever its own strip header describes.
    #[inline]
    pub fn payload_len(self) -> usize {
        match self {
            Format::FullFrame => FRAME_BYTES,
            Format::SingleFace => FACE_BYTES,
            Format::RasterStrip => STRIP_HEADER_BYTES,
        }
    }

    /// The exact payload length, where the format has one.
    #[inline]
    pub fn fixed_payload_len(self) -> Option<usize> {
        match self {
            Format::FullFrame | Format::SingleFace => Some(self.payload_len()),
            Format::RasterStrip => None,
        }
    }
}

/// The geometry in a raster strip's own header: which image, and which band of it.
///
/// `width`/`height` describe the *whole* image every strip belongs to, so a receiver
/// can size its buffer from any single strip without waiting for a particular one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strip {
    pub width: u16,
    pub height: u16,
    /// First image row this strip carries.
    pub y0: u16,
    /// How many rows it carries. Always at least 1.
    pub rows: u16,
}

impl Strip {
    /// Bytes of pixel data: `rows · width · 3`.
    #[inline]
    pub fn pixel_bytes(&self) -> usize {
        self.rows as usize * self.width as usize * 3
    }

    /// Total payload length: the strip header plus the pixels.
    #[inline]
    pub fn payload_len(&self) -> usize {
        STRIP_HEADER_BYTES + self.pixel_bytes()
    }

    /// Total datagram length, strip header and cube header included.
    #[inline]
    pub fn datagram_len(&self) -> usize {
        HEADER_BYTES + self.payload_len()
    }

    /// Every rule from the format's definition except the payload length, which only
    /// the decoder can check.
    pub fn validate(&self) -> Result<(), ProtoError> {
        if !dims_ok(self.width, self.height) {
            return Err(ProtoError::BadRasterSize {
                width: self.width,
                height: self.height,
            });
        }
        if self.rows == 0 || u32::from(self.y0) + u32::from(self.rows) > u32::from(self.height) {
            return Err(ProtoError::BadStripRange {
                y0: self.y0,
                rows: self.rows,
                height: self.height,
            });
        }
        Ok(())
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.y0.to_le_bytes());
        out.extend_from_slice(&self.rows.to_le_bytes());
    }

    fn read(payload: &[u8]) -> Strip {
        let at = |i: usize| u16::from_le_bytes([payload[i], payload[i + 1]]);
        Strip {
            width: at(0),
            height: at(2),
            y0: at(4),
            rows: at(6),
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
    /// A raster strip naming an image size outside `1..=4096` on either axis.
    BadRasterSize { width: u16, height: u16 },
    /// A raster strip whose rows fall outside the image it names.
    BadStripRange { y0: u16, rows: u16, height: u16 },
    /// A datagram larger than one UDP payload can hold.
    DatagramTooLarge { len: usize, max: usize },
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
            ProtoError::BadRasterSize { width, height } => write!(
                f,
                "raster {width}x{height} is out of range (both sides must be \
                 1..={MAX_RASTER_DIM})"
            ),
            ProtoError::BadStripRange { y0, rows, height } => write!(
                f,
                "strip rows {y0}..{} fall outside a {height}-row image (rows must be \
                 at least 1)",
                u32::from(*y0) + u32::from(*rows)
            ),
            ProtoError::DatagramTooLarge { len, max } => {
                write!(f, "datagram is {len} bytes, the limit is {max}")
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

/// Cut a raster into datagrams, each no larger than `max_datagram`, appending them to
/// `out` (cleared first). **Every strip of one image carries the same `seq`**, so a
/// receiver can tell a torn frame from a new one.
///
/// Each datagram takes the largest whole number of rows that still fits, so a
/// 320×180 image goes out in one and a 1920×1080 image in a handful. The last strip
/// is whatever is left over.
///
/// Fails only when a single row cannot fit in `max_datagram` — at the wire's own
/// [`MAX_DATAGRAM`] that is impossible (the widest legal row is 4096·3 = 12,288
/// bytes plus 24 of headers), so it can only happen with a caller-chosen limit.
pub fn encode_raster(
    raster: &Raster,
    seq: u32,
    max_datagram: usize,
    out: &mut Vec<Vec<u8>>,
) -> Result<(), ProtoError> {
    out.clear();
    let (width, height) = raster.size();
    let row_bytes = raster.row_bytes();
    let overhead = HEADER_BYTES + STRIP_HEADER_BYTES;

    let budget = max_datagram.saturating_sub(overhead);
    let rows_per = budget / row_bytes;
    if rows_per == 0 {
        return Err(ProtoError::DatagramTooLarge {
            len: overhead + row_bytes,
            max: max_datagram,
        });
    }

    let mut y0 = 0u16;
    while y0 < height {
        let rows = rows_per.min(usize::from(height - y0)) as u16;
        let strip = Strip {
            width,
            height,
            y0,
            rows,
        };
        let mut dg = Vec::with_capacity(strip.datagram_len());
        write_header(&mut dg, Format::RasterStrip, NO_FACE, seq);
        strip.write(&mut dg);
        let from = y0 as usize * row_bytes;
        dg.extend_from_slice(&raster.as_bytes()[from..from + strip.pixel_bytes()]);
        debug_assert_eq!(dg.len(), strip.datagram_len());
        out.push(dg);
        y0 += rows;
    }
    Ok(())
}

/// Split a decoded [`Format::RasterStrip`] payload into its geometry and its pixels.
///
/// [`decode`] has already validated everything this checks, so after a successful
/// `decode` this cannot fail; it is fallible so it can also be used on its own.
pub fn decode_strip(payload: &[u8]) -> Result<(Strip, &[u8]), ProtoError> {
    if payload.len() < STRIP_HEADER_BYTES {
        return Err(ProtoError::BadPayloadLen {
            expected: STRIP_HEADER_BYTES,
            got: payload.len(),
        });
    }
    let strip = Strip::read(payload);
    strip.validate()?;
    let pixels = &payload[STRIP_HEADER_BYTES..];
    if pixels.len() != strip.pixel_bytes() {
        return Err(ProtoError::BadPayloadLen {
            expected: strip.payload_len(),
            got: payload.len(),
        });
    }
    Ok((strip, pixels))
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
        2 => Format::RasterStrip,
        v => return Err(ProtoError::BadFormat(v)),
    };
    let face_byte = datagram[6];
    if datagram[7] != 0 {
        return Err(ProtoError::BadFlags(datagram[7]));
    }
    let seq = u32::from_le_bytes(datagram[8..12].try_into().unwrap());

    let face = match format {
        Format::FullFrame | Format::RasterStrip => {
            if face_byte != NO_FACE {
                return Err(ProtoError::BadFace(face_byte));
            }
            None
        }
        Format::SingleFace => Some(Face::from_index(face_byte).ok_or(ProtoError::BadFace(face_byte))?),
    };

    let payload = &datagram[HEADER_BYTES..];
    match format.fixed_payload_len() {
        // Formats 0 and 1: exactly what they have always been, byte for byte.
        Some(expected) => {
            if payload.len() != expected {
                return Err(ProtoError::BadPayloadLen {
                    expected,
                    got: payload.len(),
                });
            }
        }
        // Format 2 carries its own geometry; validate it here so that a caller's
        // `decode_strip` on the returned payload cannot fail.
        None => {
            if datagram.len() > MAX_DATAGRAM {
                return Err(ProtoError::DatagramTooLarge {
                    len: datagram.len(),
                    max: MAX_DATAGRAM,
                });
            }
            decode_strip(payload)?;
        }
    }
    // `Header` deliberately keeps its three fields: `cube-proto`'s own wire-format
    // test constructs it literally, and this format is meant to be additive. The
    // strip geometry comes from `decode_strip` on the payload this returns, which
    // starts at the strip header.
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
