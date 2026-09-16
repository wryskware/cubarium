//! Blocking UDP client.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::wire::{encode_face, encode_full, encode_raster, MAX_DATAGRAM};
use crate::{Face, Frame, ProtoError, Raster};

/// Blocking UDP sender. The socket is `connect()`ed, so `send` needs no address and the
/// OS reports ICMP errors back to us.
pub struct CubeClient {
    sock: UdpSocket,
    next_seq: u32,
    buf: Vec<u8>,
    /// Reused across `send_raster` calls so a 60 fps sender does not reallocate.
    strips: Vec<Vec<u8>>,
}

impl CubeClient {
    /// Connect to the daemon, e.g. `CubeClient::connect(("127.0.0.1", DEFAULT_PORT))`.
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<CubeClient> {
        let mut last_err: Option<io::Error> = None;
        for target in addr.to_socket_addrs()? {
            let bind: SocketAddr = if target.is_ipv4() {
                ([0, 0, 0, 0], 0).into()
            } else {
                (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
            };
            match UdpSocket::bind(bind).and_then(|s| s.connect(target).map(|()| s)) {
                Ok(sock) => {
                    return Ok(CubeClient {
                        sock,
                        next_seq: 1,
                        buf: Vec::new(),
                        strips: Vec::new(),
                    })
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "no addresses to connect to")
        }))
    }

    /// Send a whole frame. The sequence number auto-increments, starting at 1.
    pub fn send(&mut self, frame: &Frame) -> io::Result<()> {
        let seq = self.take_seq();
        encode_full(frame, seq, &mut self.buf);
        self.flush()
    }

    /// Send one face as a partial update (for senders behind a small MTU).
    pub fn send_face(&mut self, frame: &Frame, face: Face) -> io::Result<()> {
        let seq = self.take_seq();
        encode_face(frame, face, seq, &mut self.buf);
        self.flush()
    }

    /// Send a flat image as one or more raster strips. **All of them carry the same
    /// sequence number**, which is what lets the receiver tell a torn frame from the
    /// next one; the counter advances once per image, not once per datagram.
    ///
    /// A partial send is reported but the strips already out stay out: the receiver
    /// shows partial frames by design, so a dropped tail is a visual artefact, not a
    /// protocol error.
    pub fn send_raster(&mut self, raster: &Raster) -> io::Result<()> {
        let seq = self.take_seq();
        encode_raster(raster, seq, MAX_DATAGRAM, &mut self.strips).map_err(to_io)?;
        for i in 0..self.strips.len() {
            let n = self.sock.send(&self.strips[i])?;
            if n != self.strips[i].len() {
                return Err(io::Error::other(format!(
                    "short UDP send: {n} of {} bytes (strip {} of {})",
                    self.strips[i].len(),
                    i + 1,
                    self.strips.len()
                )));
            }
        }
        Ok(())
    }

    /// How many datagrams `send_raster` would use for this image.
    pub fn raster_strips(&mut self, raster: &Raster) -> Result<usize, ProtoError> {
        encode_raster(raster, 0, MAX_DATAGRAM, &mut self.strips)?;
        Ok(self.strips.len())
    }

    /// The sequence number the next datagram will carry.
    pub fn next_seq(&self) -> u32 {
        self.next_seq
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.sock
    }

    fn take_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        // Skip 0 on wrap: `seq_is_newer` treats it fine, but 1-based is the documented start.
        self.next_seq = self.next_seq.wrapping_add(1).max(1);
        seq
    }

    fn flush(&mut self) -> io::Result<()> {
        let n = self.sock.send(&self.buf)?;
        if n != self.buf.len() {
            return Err(io::Error::other(format!(
                "short UDP send: {n} of {} bytes",
                self.buf.len()
            )));
        }
        Ok(())
    }
}

fn to_io(e: ProtoError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, decode_strip, Format, DEFAULT_PORT, FRAME_BYTES, HEADER_BYTES};

    #[test]
    fn send_produces_valid_datagrams_with_increasing_seq() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let mut client = CubeClient::connect(addr).unwrap();
        assert_eq!(client.next_seq(), 1);

        let frame = Frame::black();
        client.send(&frame).unwrap();
        client.send_face(&frame, Face::Left).unwrap();

        let mut rx = vec![0u8; HEADER_BYTES + FRAME_BYTES];
        let n = server.recv(&mut rx).unwrap();
        let (h, _) = decode(&rx[..n]).unwrap();
        assert_eq!(h.seq, 1);
        assert_eq!(h.format, Format::FullFrame);

        let n = server.recv(&mut rx).unwrap();
        let (h, _) = decode(&rx[..n]).unwrap();
        assert_eq!(h.seq, 2);
        assert_eq!(h.face, Some(Face::Left));

        assert_eq!(DEFAULT_PORT, 7392);
    }

    #[test]
    fn send_raster_uses_one_seq_for_every_strip_of_an_image() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let addr = server.local_addr().unwrap();
        let mut client = CubeClient::connect(addr).unwrap();

        // The widest legal raster, so a row is 12,288 bytes and only five fit in a
        // datagram: 12 rows is three strips without being megabytes of traffic.
        let mut raster = Raster::black(4096, 12);
        raster.set(0, 0, [1, 2, 3]);
        raster.set(4095, 11, [4, 5, 6]);
        let expected = client.raster_strips(&raster).unwrap();
        assert_eq!(expected, 3, "5 + 5 + 2 rows");

        // Drain concurrently. A burst of 60 KB datagrams will overrun the default
        // socket buffer if nothing is reading — which is exactly what the daemon's
        // ingest thread is for, and what this reproduces.
        let reader = std::thread::spawn(move || {
            let mut rx = vec![0u8; 70_000];
            let mut got = Vec::new();
            for _ in 0..expected {
                let n = server.recv(&mut rx).expect("a strip arrives");
                got.push(rx[..n].to_vec());
            }
            got
        });

        client.send_raster(&raster).unwrap();
        // The next image advances the counter by exactly one, not by three.
        assert_eq!(client.next_seq(), 2);

        let got = reader.join().expect("the reader thread finishes");
        let mut rows_total = 0u32;
        let mut next_y0 = 0u16;
        for dg in &got {
            let (h, payload) = decode(dg).unwrap();
            assert_eq!(h.format, Format::RasterStrip);
            assert_eq!(h.face, None);
            assert_eq!(h.seq, 1, "every strip of one image shares its seq");
            let (strip, pixels) = decode_strip(payload).unwrap();
            assert_eq!((strip.width, strip.height), (4096, 12));
            assert_eq!(strip.y0, next_y0, "strips must tile in order with no gap");
            assert_eq!(pixels.len(), strip.pixel_bytes());
            next_y0 += strip.rows;
            rows_total += u32::from(strip.rows);
        }
        assert_eq!(rows_total, 12, "the strips must tile the image exactly");
        assert_eq!(next_y0, 12);

        // The first strip carries the top-left pixel, the last the bottom-right.
        let (_, first) = decode_strip(decode(&got[0]).unwrap().1).unwrap();
        assert_eq!(&first[..3], &[1, 2, 3]);
        let (_, last) = decode_strip(decode(&got[2]).unwrap().1).unwrap();
        assert_eq!(&last[last.len() - 3..], &[4, 5, 6]);
    }
}
