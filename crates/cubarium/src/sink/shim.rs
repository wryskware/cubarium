//! Shim sink: a worker thread owning the blocking `CubeClient`, fed through a
//! newest-frame mailbox. `submit` swaps the mailbox frame and returns; it never touches
//! the socket, so the simulation loop never waits on the network and losing the shim is
//! not fatal.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use cube_proto::{CubeClient, Frame, Raster};

use super::{FrameSink, Output};

/// First retry delay after a send or connect failure.
pub const BACKOFF_START: Duration = Duration::from_millis(100);
/// The retry delay doubles up to this ceiling.
pub const BACKOFF_MAX: Duration = Duration::from_secs(5);

/// One image waiting in the mailbox, in the shape the world has. Owned rather than
/// borrowed, because the worker reads it on its own thread long after `submit` returned.
enum Payload {
    Cube(Frame),
    Ring(Raster),
}

impl Payload {
    fn of(out: Output<'_>) -> Payload {
        match out {
            Output::Cube(f) => Payload::Cube(f.clone()),
            Output::Ring(r) => Payload::Ring(r.clone()),
        }
    }
}

#[derive(Default)]
struct Mailbox {
    slot: Mutex<Option<Payload>>,
    ready: Condvar,
}

struct Shared {
    mailbox: Mailbox,
    stop: AtomicBool,
    sent: AtomicU64,
    dropped: AtomicU64,
    errors: AtomicU64,
}

/// Sends frames to the shim daemon from a worker thread.
pub struct ShimSink {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    addr: String,
}

impl ShimSink {
    /// Start the worker. Connecting happens on the worker thread, so a shim that is down
    /// at startup is not an error.
    pub fn new(addr: impl Into<String>) -> ShimSink {
        let addr = addr.into();
        let shared = Arc::new(Shared {
            mailbox: Mailbox::default(),
            stop: AtomicBool::new(false),
            sent: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        });
        let worker = {
            let shared = Arc::clone(&shared);
            let addr = addr.clone();
            std::thread::Builder::new()
                .name("cubarium-shim".into())
                .spawn(move || worker_loop(&shared, &addr))
                .expect("spawning the shim worker thread")
        };
        ShimSink {
            shared,
            worker: Some(worker),
            addr,
        }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Datagrams the worker has handed to the socket.
    pub fn sent(&self) -> u64 {
        self.shared.sent.load(Ordering::Relaxed)
    }

    /// Frames replaced in the mailbox before the worker could send them.
    pub fn dropped(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// Send or connect failures seen by the worker.
    pub fn errors(&self) -> u64 {
        self.shared.errors.load(Ordering::Relaxed)
    }

    /// Block until the worker has drained the mailbox and sent `count` datagrams in
    /// total, or the deadline passes. Test and shutdown helper only.
    pub fn wait_for_sent(&self, count: u64, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while self.sent() < count {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.mailbox.ready.notify_all();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

impl FrameSink for ShimSink {
    fn submit(&mut self, out: Output<'_>) -> Result<()> {
        let mut slot = self
            .shared
            .mailbox
            .slot
            .lock()
            .expect("shim mailbox poisoned");
        if slot.is_some() {
            // The worker is still busy; the newest frame wins.
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
        *slot = Some(Payload::of(out));
        drop(slot);
        self.shared.mailbox.ready.notify_one();
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        // Give the worker a moment to flush whatever is in the mailbox, then stop.
        let target = self.sent() + 1;
        let _ = self.wait_for_sent(target, Duration::from_millis(200));
        self.stop();
        eprintln!(
            "cubarium: shim {}: {} sent, {} coalesced, {} errors",
            self.addr,
            self.sent(),
            self.dropped(),
            self.errors()
        );
        Ok(())
    }
}

impl Drop for ShimSink {
    fn drop(&mut self) {
        self.stop();
    }
}

fn worker_loop(shared: &Shared, addr: &str) {
    let mut client: Option<CubeClient> = None;
    let mut backoff = BACKOFF_START;
    let mut reported = false;

    loop {
        // Wait for a frame (or for shutdown).
        let frame = {
            let mut slot = match shared.mailbox.slot.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            loop {
                if shared.stop.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(f) = slot.take() {
                    break f;
                }
                let (s, _) = match shared
                    .mailbox
                    .ready
                    .wait_timeout(slot, Duration::from_millis(200))
                {
                    Ok(v) => v,
                    Err(_) => return,
                };
                slot = s;
            }
        };

        if client.is_none() {
            match CubeClient::connect(addr) {
                Ok(c) => {
                    if reported {
                        eprintln!("cubarium: reconnected to the shim at {addr}");
                        reported = false;
                    }
                    backoff = BACKOFF_START;
                    client = Some(c);
                }
                Err(e) => {
                    shared.errors.fetch_add(1, Ordering::Relaxed);
                    if !reported {
                        eprintln!("cubarium: cannot reach the shim at {addr}: {e}");
                        reported = true;
                    }
                    sleep_interruptible(shared, backoff);
                    backoff = (backoff * 2).min(BACKOFF_MAX);
                    continue;
                }
            }
        }

        if let Some(c) = client.as_mut() {
            // A cube goes out as one `encode_full` datagram (format 0); a ring goes out
            // as `encode_raster` strips (format 2), **all of them under one `seq`**, so a
            // receiver can tell a torn image from the next one. `sent` counts images, not
            // datagrams, on both paths — it is what `finish` reports and what the
            // newest-frame mailbox is about.
            let sent = match &frame {
                Payload::Cube(f) => c.send(f),
                Payload::Ring(r) => c.send_raster(r),
            };
            match sent {
                Ok(()) => {
                    if reported {
                        eprintln!("cubarium: shim send recovered");
                        reported = false;
                    }
                    backoff = BACKOFF_START;
                    shared.sent.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    shared.errors.fetch_add(1, Ordering::Relaxed);
                    if !reported {
                        eprintln!("cubarium: shim send failed: {e}");
                        reported = true;
                    }
                    // Drop the socket and reconnect on the next frame.
                    client = None;
                    sleep_interruptible(shared, backoff);
                    backoff = (backoff * 2).min(BACKOFF_MAX);
                }
            }
        }
    }
}

/// Sleep in short slices so shutdown never waits out a five-second backoff.
fn sleep_interruptible(shared: &Shared, total: Duration) {
    let slice = Duration::from_millis(20);
    let mut left = total;
    while left > Duration::ZERO {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        let d = left.min(slice);
        std::thread::sleep(d);
        left -= d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_never_blocks_when_the_shim_is_down() {
        // Port 1 on the loopback has nothing listening; connect succeeds (UDP) but sends
        // draw ICMP errors. Either way the loop must keep running.
        let mut sink = ShimSink::new("127.0.0.1:1");
        let frame = Frame::black();
        let t0 = std::time::Instant::now();
        for _ in 0..50 {
            sink.submit(Output::Cube(&frame)).unwrap();
        }
        assert!(
            t0.elapsed() < Duration::from_millis(500),
            "submit blocked on I/O"
        );
    }

    #[test]
    fn an_unresolvable_address_is_not_fatal() {
        let mut sink = ShimSink::new("this-host-does-not-exist.invalid:7392");
        let frame = Frame::black();
        for _ in 0..5 {
            sink.submit(Output::Cube(&frame)).unwrap();
        }
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            sink.errors() > 0,
            "the worker should have reported a connect failure"
        );
        sink.finish().unwrap();
    }

    /// A ring world goes through the same mailbox, the same worker and the same backoff:
    /// only the datagram the worker writes changes.
    #[test]
    fn a_ring_raster_goes_through_the_same_mailbox() {
        let mut sink = ShimSink::new("127.0.0.1:1");
        let raster = Raster::black(320, 180);
        let t0 = std::time::Instant::now();
        for _ in 0..50 {
            sink.submit(Output::Ring(&raster)).unwrap();
        }
        assert!(
            t0.elapsed() < Duration::from_millis(500),
            "submit blocked on I/O"
        );
    }
}
