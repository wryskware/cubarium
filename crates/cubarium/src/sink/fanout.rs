//! Fan-out sink: one encoded frame, several destinations.
//!
//! `--mirror-web` puts the shim (or the preview, or the png writer) and the loopback
//! viewer behind one [`FrameSink`], so the host still renders once per frame and both
//! destinations receive the *identical* `Frame` bytes. Nothing here re-encodes, re-renders
//! or copies pixels: every child is handed the same `&Frame`.
//!
//! Failure policy. A child that fails must not silently cancel the others — a browser tab
//! that went away is not a reason to stop feeding the cube — so `submit` and `finish`
//! always visit every child and return the first error afterwards. `should_quit` is the
//! disjunction: the preview window closing still stops the host.

use anyhow::Result;
use cube_proto::Frame;

use super::FrameSink;

/// Hands each frame to every child sink in order.
pub struct FanOutSink(Vec<Box<dyn FrameSink>>);

impl FanOutSink {
    pub fn new(sinks: Vec<Box<dyn FrameSink>>) -> FanOutSink {
        FanOutSink(sinks)
    }

    /// How many sinks the frame reaches.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FrameSink for FanOutSink {
    fn submit(&mut self, frame: &Frame) -> Result<()> {
        let mut first: Option<anyhow::Error> = None;
        for sink in &mut self.0 {
            // The same borrow every time: no child can observe different bytes.
            if let Err(e) = sink.submit(frame)
                && first.is_none()
            {
                first = Some(e);
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn observe_tick(&mut self, tick: u64) {
        for sink in &mut self.0 {
            sink.observe_tick(tick);
        }
    }

    fn should_quit(&mut self) -> bool {
        // Every child is polled, not just up to the first `true`: a sink's `should_quit`
        // is also how it drains its own event queue.
        let mut quit = false;
        for sink in &mut self.0 {
            quit |= sink.should_quit();
        }
        quit
    }

    fn finish(&mut self) -> Result<()> {
        let mut first: Option<anyhow::Error> = None;
        for sink in &mut self.0 {
            if let Err(e) = sink.finish()
                && first.is_none()
            {
                first = Some(e);
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Records what it was given, and can be told to fail.
    struct Probe {
        seen: Arc<AtomicU64>,
        ticks: Arc<AtomicU64>,
        quit: bool,
        fail: Option<&'static str>,
        got: Arc<std::sync::Mutex<Vec<Frame>>>,
    }

    impl Probe {
        fn new() -> Probe {
            Probe {
                seen: Arc::new(AtomicU64::new(0)),
                ticks: Arc::new(AtomicU64::new(0)),
                quit: false,
                fail: None,
                got: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    impl FrameSink for Probe {
        fn submit(&mut self, frame: &Frame) -> Result<()> {
            self.seen.fetch_add(1, Ordering::Relaxed);
            self.got.lock().unwrap().push(frame.clone());
            match self.fail {
                Some(m) => Err(anyhow::anyhow!("{m}")),
                None => Ok(()),
            }
        }

        fn observe_tick(&mut self, tick: u64) {
            self.ticks.store(tick, Ordering::Relaxed);
        }

        fn should_quit(&mut self) -> bool {
            self.quit
        }

        fn finish(&mut self) -> Result<()> {
            match self.fail {
                Some(m) => Err(anyhow::anyhow!("finish {m}")),
                None => Ok(()),
            }
        }
    }

    fn frame(seed: u8) -> Frame {
        let mut f = Frame::black();
        for (i, b) in f.as_bytes_mut().iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(11).wrapping_add(seed);
        }
        f
    }

    #[test]
    fn every_child_sees_the_same_bytes() {
        let (a, b) = (Probe::new(), Probe::new());
        let (got_a, got_b) = (Arc::clone(&a.got), Arc::clone(&b.got));
        let mut fan = FanOutSink::new(vec![Box::new(a), Box::new(b)]);
        assert_eq!(fan.len(), 2);
        for seed in 0..4u8 {
            fan.submit(&frame(seed)).unwrap();
        }
        let (ga, gb) = (got_a.lock().unwrap(), got_b.lock().unwrap());
        assert_eq!(ga.len(), 4);
        assert_eq!(gb.len(), 4);
        for (i, (x, y)) in ga.iter().zip(gb.iter()).enumerate() {
            assert_eq!(x.as_bytes().as_slice(), frame(i as u8).as_bytes().as_slice());
            assert_eq!(x.as_bytes().as_slice(), y.as_bytes().as_slice(), "frame {i} differs");
        }
    }

    #[test]
    fn a_failing_child_does_not_cancel_the_others_and_the_first_error_wins() {
        let mut a = Probe::new();
        a.fail = Some("first sink is down");
        let mut b = Probe::new();
        b.fail = Some("second sink is down");
        let c = Probe::new();
        let (seen_b, seen_c) = (Arc::clone(&b.seen), Arc::clone(&c.seen));
        let mut fan = FanOutSink::new(vec![Box::new(a), Box::new(b), Box::new(c)]);

        let err = fan.submit(&frame(1)).unwrap_err().to_string();
        assert_eq!(err, "first sink is down", "the first error propagates");
        assert_eq!(seen_b.load(Ordering::Relaxed), 1, "the second child was still tried");
        assert_eq!(seen_c.load(Ordering::Relaxed), 1, "the third child was still tried");

        let err = fan.finish().unwrap_err().to_string();
        assert_eq!(err, "finish first sink is down");
    }

    #[test]
    fn quitting_is_any_child_and_the_tick_reaches_all_of_them() {
        let a = Probe::new();
        let mut b = Probe::new();
        b.quit = true;
        let (ticks_a, ticks_b) = (Arc::clone(&a.ticks), Arc::clone(&b.ticks));
        let mut fan = FanOutSink::new(vec![Box::new(a), Box::new(b)]);
        assert!(fan.should_quit(), "one child asking to quit stops the host");
        fan.observe_tick(4242);
        assert_eq!(ticks_a.load(Ordering::Relaxed), 4242);
        assert_eq!(ticks_b.load(Ordering::Relaxed), 4242);
    }
}
