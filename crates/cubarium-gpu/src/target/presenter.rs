//! Presentation on its own thread: the channel between the loop that records frames and
//! the thread that puts them on the panel.
//!
//! # Why
//!
//! The board's main thread packed a tick (7 ms), recorded a frame, submitted it, waited
//! for its fence (8 ms) and only then went back to the simulation, so the panel ran at
//! 19–21 fps against a 60 Hz display. Everything after the recording is waiting, and
//! waiting is what a second thread is for.
//!
//! # The split
//!
//! * The **recorder** — the run loop — owns the renderer: it packs the world, records a
//!   command buffer into a slot the presenter lent it, and posts it. It never blocks:
//!   with no free slot it drops the frame, and a frame that is still waiting to go up is
//!   *replaced* rather than queued behind.
//! * The **presenter** owns the queue submit, the fence wait, the socket and the free
//!   mask. It tells the recorder two things: that a frame has retired (so the resources
//!   it named may be written again) and that a slot is free (so it may be drawn into).
//!
//! Nothing here knows about Vulkan or about the daemon's wire: [`Panel`] is what the
//! thread does to the panel, and [`super::shim::ShimScanout`] is the one that really does
//! it. That is what lets the semantics — newest frame wins, one frame in flight, a clean
//! shutdown — be tested against a fake in microseconds.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex};

use anyhow::Result;

/// What the presenter thread does with a recorded frame.
///
/// Slots are **indices**, `0..slots`, and not the daemon's slot ids: the ids change every
/// time the panel changes hands and the index does not.
pub trait Panel: Send {
    /// Submit the command buffer recorded into `slot` and wait for its fence. The wire
    /// carries no fence, so the daemon must not be shown a frame the GPU has not
    /// finished — the wait stays.
    fn submit_and_wait(&mut self, slot: usize) -> Result<()>;

    /// Show `slot`. It belongs to the daemon until it releases it.
    fn present(&mut self, slot: usize) -> Result<()>;

    /// The slots the daemon has released since the last call, and which are therefore
    /// free to be drawn into again. With `block`, wait for a reply when none is free;
    /// without it, report only what is already known.
    fn released(&mut self, block: bool) -> Result<Vec<usize>>;
}

/// What the presenter tells the recorder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromPresenter {
    /// The oldest outstanding frame has completed on the GPU. What it named — the staging
    /// buffer it uploaded from, its uniform block — may be written again.
    Retired,
    /// This slot may be recorded into.
    Free(usize),
    /// The presenter has stopped, and why. The recorder fails the run on it, exactly as
    /// a synchronous present's error used to fail it.
    Failed(String),
}

/// The one frame waiting to be presented.
///
/// **Newest wins, one in flight, and neither side waits for the other.** A recorder that
/// posts while a frame is still waiting displaces it: the panel should show the freshest
/// world, and a queue of frames behind a slow GPU is a queue of stale pictures. The
/// recorder takes the displaced frame back and owes its slot and its staging buffer to
/// nobody.
#[derive(Debug, Default)]
struct Posted {
    frame: Option<usize>,
    quit: bool,
}

/// The recorder's end and the presenter's end of that one frame.
#[derive(Debug)]
pub struct Mailbox {
    state: Mutex<Posted>,
    posted: Condvar,
}

impl Default for Mailbox {
    fn default() -> Mailbox {
        Mailbox::new()
    }
}

impl Mailbox {
    pub fn new() -> Mailbox {
        Mailbox {
            state: Mutex::new(Posted::default()),
            posted: Condvar::new(),
        }
    }

    /// Put `slot` up to be presented. Returns the frame it displaced, which is the
    /// recorder's again — its slot is free and its recording will never be submitted.
    pub fn post(&self, slot: usize) -> Option<usize> {
        let mut state = self.lock();
        let displaced = state.frame.replace(slot);
        drop(state);
        self.posted.notify_one();
        displaced
    }

    /// Take back the waiting frame, if the presenter has not started on it yet.
    ///
    /// The recorder calls this **before** it records a fresher frame, so that the frame
    /// it gives up is always the newest one recorded: the renderer's ring retires frames
    /// in order and can only give back the last thing it was handed.
    pub fn reclaim(&self) -> Option<usize> {
        self.lock().frame.take()
    }

    /// The presenter's wait: the next frame, or `None` once the run is over.
    pub fn take(&self) -> Option<usize> {
        let mut state = self.lock();
        loop {
            if let Some(slot) = state.frame.take() {
                return Some(slot);
            }
            if state.quit {
                return None;
            }
            state = self
                .posted
                .wait(state)
                .expect("the mailbox lock is never poisoned");
        }
    }

    /// End the run. A frame already posted still goes up — the recorder has stopped
    /// recording by then, so it is one frame and not a backlog — and the presenter stops
    /// as soon as the mailbox is empty.
    pub fn quit(&self) {
        self.lock().quit = true;
        self.posted.notify_all();
    }

    /// Whether the run is over, for a presenter about to block on the daemon.
    pub fn quitting(&self) -> bool {
        self.lock().quit
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Posted> {
        self.state
            .lock()
            .expect("the mailbox lock is never poisoned")
    }
}

/// The presenter thread's whole body.
///
/// It lends the recorder every slot the daemon is not using, waits for a frame, submits
/// it, waits for its fence, says so, presents it, and goes looking for the next free
/// slot. Blocking for a reply is right where it happens: nothing is waiting on this
/// thread, and a reply is the only thing that frees a slot.
pub fn present_loop<P: Panel>(
    panel: &mut P,
    mail: &Mailbox,
    back: &Sender<FromPresenter>,
    presented: &AtomicU64,
    lent: Vec<usize>,
) -> Result<()> {
    // Slots the recorder has, or has posted back and not yet had presented. It starts
    // holding every slot — nothing has been presented, so the daemon is using none — and
    // this is also what keeps a re-attach from lending one twice: the daemon forgets
    // every slot when the panel changes hands and offers them all again, and these are
    // the ones that are not its to offer.
    let mut lent = lent;
    while let Some(slot) = mail.take() {
        panel.submit_and_wait(slot)?;
        // The fence is signalled: the staging buffer and the uniform block this frame
        // named are the recorder's to write again. This goes back before the present, so
        // the next pack is not waiting on a vsync.
        if back.send(FromPresenter::Retired).is_err() {
            break;
        }
        panel.present(slot)?;
        lent.retain(|s| *s != slot);
        presented.fetch_add(1, Ordering::Relaxed);
        // **Only a starved recorder is worth sleeping for.** A reply arrives when the
        // daemon's flip completes, so waiting for one while the recorder still has a slot
        // would put a whole vsync between this frame and the next — 24 ms a frame instead
        // of 16. With three slots the recorder runs out every third frame, which is often
        // enough that the replies never pile up. A shutting-down recorder is not waiting
        // for anything, and the daemon may be the thing that went away.
        let block = lent.is_empty() && !mail.quitting();
        if !lend(panel.released(block)?, &mut lent, back) {
            break;
        }
    }
    Ok(())
}

/// Hand every slot that is not already out to the recorder. False once the recorder has
/// gone away, which is a shutdown and not a failure.
fn lend(free: Vec<usize>, lent: &mut Vec<usize>, back: &Sender<FromPresenter>) -> bool {
    for slot in free {
        if lent.contains(&slot) {
            continue;
        }
        lent.push(slot);
        if back.send(FromPresenter::Free(slot)).is_err() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    /// A panel that records what it was asked to do and answers from a script.
    #[derive(Default)]
    struct FakePanel {
        log: Vec<String>,
        /// What `released` hands back, one call at a time.
        releases: std::collections::VecDeque<Vec<usize>>,
        fail_on_submit: Option<usize>,
    }

    impl Panel for FakePanel {
        fn submit_and_wait(&mut self, slot: usize) -> Result<()> {
            self.log.push(format!("submit {slot}"));
            if self.fail_on_submit == Some(slot) {
                anyhow::bail!("the device is gone");
            }
            Ok(())
        }
        fn present(&mut self, slot: usize) -> Result<()> {
            self.log.push(format!("present {slot}"));
            Ok(())
        }
        fn released(&mut self, block: bool) -> Result<Vec<usize>> {
            self.log.push(format!("released block={block}"));
            Ok(self.releases.pop_front().unwrap_or_default())
        }
    }

    /// The panel should show the freshest world. A frame that has not been taken yet is
    /// displaced by the next one and handed back to the recorder, rather than queueing up
    /// behind it — a queue of frames is a queue of stale pictures.
    #[test]
    fn the_newest_frame_displaces_the_one_still_waiting() {
        let mail = Mailbox::new();
        assert_eq!(mail.post(0), None, "nothing was waiting");
        assert_eq!(mail.post(1), Some(0), "the first frame is handed back");
        assert_eq!(mail.post(2), Some(1));
        assert_eq!(mail.take(), Some(2), "the presenter sees only the newest");
    }

    /// The recorder takes its own frame back before recording a fresher one, so the frame
    /// it gives up is always the last one recorded — which is the only one the renderer's
    /// ring can give back.
    #[test]
    fn a_frame_the_presenter_has_not_started_is_reclaimed() {
        let mail = Mailbox::new();
        assert_eq!(mail.reclaim(), None, "nothing to reclaim");
        mail.post(2);
        assert_eq!(mail.reclaim(), Some(2));
        assert_eq!(mail.reclaim(), None, "and only once");
    }

    /// Neither posting nor reclaiming ever waits for the presenter: both are a lock and
    /// an `Option`. (If either blocked, this test would hang rather than fail.)
    #[test]
    fn the_recorder_never_waits_on_the_presenter() {
        let mail = Mailbox::new();
        for i in 0..1000 {
            mail.post(i % 3);
            mail.reclaim();
        }
        assert_eq!(mail.take_now(), None);
    }

    /// One frame in flight: submit, say it has retired, present, then look for the next
    /// free slot — in that order, because the recorder should get its staging buffer back
    /// without waiting for a vsync.
    #[test]
    fn a_frame_is_submitted_retired_and_then_presented() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let presented = AtomicU64::new(0);
        let mut panel = FakePanel {
            releases: [vec![]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(0);
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &presented, vec![0, 1, 2]).unwrap();

        assert_eq!(
            panel.log,
            vec![
                "submit 0",
                "present 0",
                // The recorder still holds 1 and 2, so nothing sleeps on a reply.
                "released block=false",
            ]
        );
        assert_eq!(presented.load(Ordering::Relaxed), 1);
        let got: Vec<FromPresenter> = rx.try_iter().collect();
        assert_eq!(got, vec![FromPresenter::Retired]);
    }

    /// The daemon forgets every slot when the panel changes hands and offers them all
    /// again on the re-attach. The ones the recorder is already holding must not be
    /// handed to it a second time.
    #[test]
    fn a_slot_the_recorder_already_holds_is_not_lent_twice() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let presented = AtomicU64::new(0);
        let mut panel = FakePanel {
            // The re-attach: the daemon has forgotten everything and offers it all.
            releases: [vec![0, 1, 2]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(1);
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &presented, vec![0, 1, 2]).unwrap();

        let free: Vec<usize> = rx
            .try_iter()
            .filter_map(|m| match m {
                FromPresenter::Free(s) => Some(s),
                _ => None,
            })
            .collect();
        // Only 1 comes back: it was presented, so the daemon had it and gave it back.
        assert_eq!(free, vec![1], "0 and 2 are still the recorder's");
    }

    /// Quitting ends the thread with nothing left to present — it does not wait for a
    /// frame that is never coming. (If it did, this test would hang.)
    #[test]
    fn the_thread_stops_on_quit() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let presented = AtomicU64::new(0);
        let mut panel = FakePanel::default();
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &presented, vec![0]).unwrap();
        assert_eq!(presented.load(Ordering::Relaxed), 0, "nothing went up");
        assert!(panel.log.is_empty(), "{:?}", panel.log);
        drop(rx);
    }

    /// A real shutdown races: the recorder posts a frame and quits while the presenter is
    /// still working. The thread drains the one frame it was handed and then stops, so a
    /// join never waits on a frame and never loses one silently.
    #[test]
    fn a_frame_posted_as_the_run_ends_is_still_presented_and_then_the_thread_stops() {
        let mail = std::sync::Arc::new(Mailbox::new());
        let (tx, rx) = channel();
        let presented = std::sync::Arc::new(AtomicU64::new(0));
        let (m, p) = (mail.clone(), presented.clone());
        let thread = std::thread::spawn(move || {
            let mut panel = FakePanel {
                releases: [vec![0]].into_iter().collect(),
                ..FakePanel::default()
            };
            present_loop(&mut panel, &m, &tx, &p, vec![0, 1]).unwrap();
            panel.log
        });
        mail.post(1);
        mail.quit();
        // The thread ran: it says the frame's resources are free again.
        assert_eq!(rx.recv().unwrap(), FromPresenter::Retired);
        let log = thread.join().expect("the presenter thread ends");
        assert_eq!(
            presented.load(Ordering::Relaxed),
            1,
            "the last frame went up"
        );
        assert!(log.contains(&"present 1".to_string()), "{log:?}");
    }

    /// A panel that fails ends the loop with its error, which the recorder turns into the
    /// run's failure — the same end a synchronous present had.
    #[test]
    fn a_failing_panel_ends_the_loop() {
        let mail = Mailbox::new();
        let (tx, _rx) = channel();
        let presented = AtomicU64::new(0);
        let mut panel = FakePanel {
            fail_on_submit: Some(0),
            ..FakePanel::default()
        };
        mail.post(0);
        mail.quit();
        let e = present_loop(&mut panel, &mail, &tx, &presented, vec![0]).expect_err("it failed");
        assert!(format!("{e:#}").contains("the device is gone"), "{e:#}");
    }

    /// A recorder that has gone away (its receiver dropped) is a shutdown, not a failure:
    /// the frame it left behind is submitted and the thread ends without an error.
    #[test]
    fn a_gone_recorder_ends_the_loop_quietly() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        drop(rx);
        let presented = AtomicU64::new(0);
        let mut panel = FakePanel::default();
        mail.post(0);
        present_loop(&mut panel, &mail, &tx, &presented, vec![0]).expect("no error");
        assert_eq!(panel.log, vec!["submit 0"], "it stopped at the first send");
    }
}

#[cfg(test)]
impl Mailbox {
    /// The waiting frame without waiting for one, for the tests.
    fn take_now(&self) -> Option<usize> {
        self.lock().frame.take()
    }
}
