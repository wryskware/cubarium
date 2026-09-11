//! Blocking UDP client.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::wire::{encode_face, encode_full};
use crate::{Face, Frame};

/// Blocking UDP sender. The socket is `connect()`ed, so `send` needs no address and the
/// OS reports ICMP errors back to us.
pub struct CubeClient {
    sock: UdpSocket,
    next_seq: u32,
    buf: Vec<u8>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, Format, DEFAULT_PORT, FRAME_BYTES, HEADER_BYTES};

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
}
