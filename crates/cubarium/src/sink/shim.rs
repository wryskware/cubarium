//! Shim sink: a worker thread owning the blocking `CubeClient`, fed through a
//! newest-frame mailbox. `submit` swaps the mailbox frame and returns; it never touches
//! the socket, so the simulation loop never waits on the network and losing the shim is
//! not fatal.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use cube_proto::{CubeClient, Frame};

use super::FrameSink;

/// First retry delay after a send or connect failure.
pub const BACKOFF_START: Duration = Duration::from_millis(100);
/// The retry delay doubles up to this ceiling.
pub const BACKOFF_MAX: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Mailbox {
    slot: Mutex<Option<Frame>>,
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
        ShimSink { shared, worker: Some(worker), addr }
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
    fn submit(&mut self, frame: &Frame) -> Result<()> {
        let mut slot = self.shared.mailbox.slot.lock().expect("shim mailbox poisoned");
        if slot.is_some() {
            // The worker is still busy; the newest frame wins.
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
        *slot = Some(frame.clone());
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
            match c.send(&frame) {
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
            sink.submit(&frame).unwrap();
        }
        assert!(t0.elapsed() < Duration::from_millis(500), "submit blocked on I/O");
    }

    #[test]
    fn an_unresolvable_address_is_not_fatal() {
        let mut sink = ShimSink::new("this-host-does-not-exist.invalid:7392");
        let frame = Frame::black();
        for _ in 0..5 {
            sink.submit(&frame).unwrap();
        }
        std::thread::sleep(Duration::from_millis(150));
        assert!(sink.errors() > 0, "the worker should have reported a connect failure");
        sink.finish().unwrap();
    }
}
