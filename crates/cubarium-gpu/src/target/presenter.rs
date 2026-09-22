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
//!
//! # Why it polls while it waits
//!
//! The daemon reports a freed slot **only** in the `Presented` reply it sends when a flip
//! completes (`led-cube-shim`, `handoff/slots.rs`: `flip_complete` drains the released
//! set into that one reply), so a slot the panel finished with is not free until this
//! thread reads the socket. The first version read it only after a present, and only when
//! it had nothing left to lend — so a release that arrived while the thread sat in
//! `Mailbox::take` waited there until the next frame happened to come. That is a cycle the
//! recorder cannot get out of: it has no slot, so it records nothing; nothing is posted,
//! so this thread keeps waiting; and the reply that would have broken it is already in the
//! socket, unread. The board showed it exactly — every present starved of a slot, idle
//! climbing 16.6 → 34.8 ms as the main loop slowed, and the panel falling 52 → 22 fps.
//!
//! So the wait is bounded: the thread drains the socket before it waits and every few
//! milliseconds while it waits, and a released slot reaches the recorder within that poll
//! rather than within a frame.
//!
//! # Where the time goes
//!
//! The board's first reading with the thread in place: 50.9 frames a second recorded,
//! **19.9 presented**, and the presenter thread at 81 % of a core. Eighty-one per cent of
//! a core for twenty presents is forty milliseconds of *running* per frame, and a thread
//! blocked on a fence or on a socket is not running — so the cost is inside the driver,
//! not in this channel. [`PresentStats`] is therefore not decoration: every phase of a
//! present is timed and reported, and frames that redrew the world are timed apart from
//! frames that only put an already-drawn raster on a new slot. One reading of that line
//! says whether the panel's ceiling is the world pass, the upscale onto 1080×1920, the
//! daemon's pacing, or this side having nothing to give it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use anyhow::Result;

/// A recorded frame, on its way to the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// The slot its command buffer was recorded into.
    pub slot: usize,
    /// Whether it redrew the world raster, or only put the last one on a new slot. The
    /// two cost very different amounts and are reported apart.
    pub redrew: bool,
    /// The renderer's content version when it was recorded, if the renderer keeps one.
    /// A frame still waiting whose version is the current one would be re-recorded into
    /// the same commands, so it is left alone instead.
    pub version: Option<u64>,
}

/// What the recorder should do this frame, decided before any Vulkan is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// A frame is already waiting that shows this same world: leave it there. Recording
    /// it again would build the same command buffer out of the same texture, on the one
    /// thread that has a simulation to run.
    Keep,
    /// Nowhere to put a frame: no slot free, or the renderer already holding as many as
    /// it has per-frame resources for.
    Drop,
    /// The waiting frame is stale: take it back — its slot with it — and record the
    /// fresher world into that same slot.
    Replace { slot: usize },
    /// Nothing waiting: record into this slot.
    Record { slot: usize },
}

/// The recorder's whole policy, as a function of what it can see.
///
/// `pending` is the frame still waiting to be presented, `version` what the renderer
/// would record now, `free` the slot it would use, and `in_flight`/`capacity` the
/// renderer's per-frame resources. Pure, so the rules are tested rather than inferred
/// from a board reading.
pub fn next_frame(
    pending: Option<Frame>,
    version: Option<u64>,
    free: Option<usize>,
    in_flight: usize,
    capacity: usize,
) -> Next {
    if let Some(waiting) = pending {
        // **Newest wins, but only when there is a newer one.** A renderer that does not
        // version its content never matches, so it is replaced as before.
        if waiting.version.is_some() && waiting.version == version {
            return Next::Keep;
        }
        // Taking it back frees its ring entry as well as its slot.
        return if in_flight.saturating_sub(1) >= capacity {
            Next::Drop
        } else {
            Next::Replace { slot: waiting.slot }
        };
    }
    match free {
        Some(slot) if in_flight < capacity => Next::Record { slot },
        _ => Next::Drop,
    }
}

/// What the presenter thread does with a recorded frame.
///
/// Slots are **indices**, `0..slots`, and not the daemon's slot ids: the ids change every
/// time the panel changes hands and the index does not.
pub trait Panel: Send {
    /// Submit the command buffer recorded into `slot`.
    fn submit(&mut self, slot: usize) -> Result<()>;

    /// Wait for that submission's fence. The wire carries no fence, so the daemon must
    /// not be shown a frame the GPU has not finished — the wait stays. It is its own call
    /// so that the submit and the wait can be told apart in the report.
    fn wait(&mut self, slot: usize) -> Result<()>;

    /// Show `slot`. It belongs to the daemon until it releases it.
    fn present(&mut self, slot: usize) -> Result<()>;

    /// The slots the daemon has released since the last call, and which are therefore
    /// free to be drawn into again. With `block`, wait for a reply when none is free;
    /// without it, report only what has already arrived.
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

/// Every phase of every present, in nanoseconds, as the presenter thread accumulates it.
///
/// Read through [`PresentStats::snapshot`]; the numbers that matter are the differences
/// between two snapshots, which is what the running report prints.
#[derive(Debug, Default)]
pub struct PresentStats {
    presented: AtomicU64,
    redrawn: AtomicU64,
    /// Waiting for the recorder to post anything: the panel is ahead of the world.
    idle_ns: AtomicU64,
    submit_ns: AtomicU64,
    fence_ns: AtomicU64,
    /// `sendmsg(Present)`.
    show_ns: AtomicU64,
    /// Reading the daemon's replies, including waiting for one when no slot is free.
    slots_ns: AtomicU64,
    /// Submit plus fence, split by what the frame did.
    redraw_ns: AtomicU64,
    reshow_ns: AtomicU64,
    /// Presents that had to wait on the daemon because the recorder held no slot.
    starved: AtomicU64,
    /// Slots the recorder was holding at each present, summed: the pipeline's depth, and
    /// the number that says whether the panel's rate is a slot supply problem.
    lent_sum: AtomicU64,
}

impl PresentStats {
    fn add(&self, field: &AtomicU64, ns: u64) {
        field.fetch_add(ns, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> PresentSample {
        let g = |f: &AtomicU64| f.load(Ordering::Relaxed);
        PresentSample {
            presented: g(&self.presented),
            redrawn: g(&self.redrawn),
            idle_ns: g(&self.idle_ns),
            submit_ns: g(&self.submit_ns),
            fence_ns: g(&self.fence_ns),
            show_ns: g(&self.show_ns),
            slots_ns: g(&self.slots_ns),
            redraw_ns: g(&self.redraw_ns),
            reshow_ns: g(&self.reshow_ns),
            starved: g(&self.starved),
            lent_sum: g(&self.lent_sum),
            held: 0,
            skipped: 0,
            refused_packs: 0,
        }
    }

    pub fn presented(&self) -> u64 {
        self.presented.load(Ordering::Relaxed)
    }
}

/// Two snapshots apart: what the presenter did over one interval.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PresentSample {
    pub presented: u64,
    pub redrawn: u64,
    pub idle_ns: u64,
    pub submit_ns: u64,
    pub fence_ns: u64,
    pub show_ns: u64,
    pub slots_ns: u64,
    pub redraw_ns: u64,
    pub reshow_ns: u64,
    pub starved: u64,
    /// Slots the recorder held at each present, summed.
    pub lent_sum: u64,
    /// Frames the recorder did not record because the one already waiting showed the
    /// same world.
    pub held: u64,
    /// Frames the **recorder** could not place, filled in by the recorder's own side.
    pub skipped: u64,
    /// Packs the renderer refused because every staging buffer was still being read.
    pub refused_packs: u64,
}

impl PresentSample {
    /// This sample minus an earlier one: the interval between them.
    pub fn since(&self, earlier: &PresentSample) -> PresentSample {
        let d = |a: u64, b: u64| a.saturating_sub(b);
        PresentSample {
            presented: d(self.presented, earlier.presented),
            redrawn: d(self.redrawn, earlier.redrawn),
            idle_ns: d(self.idle_ns, earlier.idle_ns),
            submit_ns: d(self.submit_ns, earlier.submit_ns),
            fence_ns: d(self.fence_ns, earlier.fence_ns),
            show_ns: d(self.show_ns, earlier.show_ns),
            slots_ns: d(self.slots_ns, earlier.slots_ns),
            redraw_ns: d(self.redraw_ns, earlier.redraw_ns),
            reshow_ns: d(self.reshow_ns, earlier.reshow_ns),
            starved: d(self.starved, earlier.starved),
            lent_sum: d(self.lent_sum, earlier.lent_sum),
            held: d(self.held, earlier.held),
            skipped: d(self.skipped, earlier.skipped),
            refused_packs: d(self.refused_packs, earlier.refused_packs),
        }
    }

    /// One line: where a present's time went, and what never got to be one.
    ///
    /// Every millisecond figure is **per present**, so they add up to the interval the
    /// thread had per frame. A thread that is not the ceiling shows most of it as idle.
    pub fn line(&self, seconds: f64, device: &str) -> String {
        let per = |ns: u64| {
            if self.presented == 0 {
                0.0
            } else {
                ns as f64 / self.presented as f64 / 1e6
            }
        };
        let each = |ns: u64, n: u64| {
            if n == 0 {
                f64::NAN
            } else {
                ns as f64 / n as f64 / 1e6
            }
        };
        let reshown = self.presented.saturating_sub(self.redrawn);
        format!(
            "presenter — {} shown ({:.1}/s), {} redrew the world; per present idle {:.1}, \
             submit {:.1}, fence {:.1}, show {:.1}, slots {:.1} ms; \
             redraw {:.1} ms vs re-present {:.1} ms ({reshown}); \
             {} frames had no slot, {} already current, {} packs refused, {} starved of a slot, \
             {:.1} slots in hand; on {device}",
            self.presented,
            self.presented as f64 / seconds.max(1e-9),
            self.redrawn,
            per(self.idle_ns),
            per(self.submit_ns),
            per(self.fence_ns),
            per(self.show_ns),
            per(self.slots_ns),
            each(self.redraw_ns, self.redrawn),
            each(self.reshow_ns, reshown),
            self.skipped,
            self.held,
            self.refused_packs,
            self.starved,
            if self.presented == 0 {
                0.0
            } else {
                self.lent_sum as f64 / self.presented as f64
            },
        )
    }
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
    frame: Option<Frame>,
    quit: bool,
}

/// What a bounded wait on the mailbox found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    Frame(Frame),
    /// Nothing yet — go and read the socket, then come back.
    Nothing,
    Quit,
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

    /// Put `frame` up to be presented. Returns the frame it displaced, which is the
    /// recorder's again — its slot is free and its recording will never be submitted.
    pub fn post(&self, frame: Frame) -> Option<Frame> {
        let mut state = self.lock();
        let displaced = state.frame.replace(frame);
        drop(state);
        self.posted.notify_one();
        displaced
    }

    /// The frame still waiting, without taking it.
    pub fn pending(&self) -> Option<Frame> {
        self.lock().frame
    }

    /// Take back the waiting frame, if the presenter has not started on it yet.
    ///
    /// The recorder calls this **before** it records a fresher frame, so that the frame
    /// it gives up is always the newest one recorded: the renderer's ring retires frames
    /// in order and can only give back the last thing it was handed.
    pub fn reclaim(&self) -> Option<Frame> {
        self.lock().frame.take()
    }

    /// The presenter's wait: the next frame, or `None` once the run is over.
    pub fn take(&self) -> Option<Frame> {
        loop {
            match self.take_until(std::time::Duration::from_millis(100)) {
                Waited::Frame(frame) => return Some(frame),
                Waited::Quit => return None,
                Waited::Nothing => continue,
            }
        }
    }

    /// The same, but giving up after `poll` so the caller can go and read the socket.
    ///
    /// **The daemon reports a freed slot only in a reply**, and a reply only arrives when
    /// a flip completes. A presenter asleep here is a presenter not reading them, and a
    /// recorder with no slot cannot post the frame that would wake it.
    pub fn take_until(&self, poll: std::time::Duration) -> Waited {
        let mut state = self.lock();
        loop {
            if let Some(frame) = state.frame.take() {
                return Waited::Frame(frame);
            }
            if state.quit {
                return Waited::Quit;
            }
            let (next, timeout) = self
                .posted
                .wait_timeout(state, poll)
                .expect("the mailbox lock is never poisoned");
            state = next;
            if timeout.timed_out() {
                // One more look under the lock before giving the caller its turn.
                if let Some(frame) = state.frame.take() {
                    return Waited::Frame(frame);
                }
                return if state.quit {
                    Waited::Quit
                } else {
                    Waited::Nothing
                };
            }
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
/// It waits for a frame, submits it, waits for its fence, says so, presents it, and goes
/// looking for the next free slot. Every one of those is timed into `stats`: the thread
/// that turns out to be the panel's ceiling should be able to say which part of itself is
/// the ceiling, rather than leaving it to be inferred from a load average.
pub fn present_loop<P: Panel>(
    panel: &mut P,
    mail: &Mailbox,
    back: &Sender<FromPresenter>,
    stats: &PresentStats,
    lent: Vec<usize>,
    poll: std::time::Duration,
) -> Result<()> {
    // Slots the recorder has, or has posted back and not yet had presented. It starts
    // holding every slot — nothing has been presented, so the daemon is using none — and
    // this is also what keeps a re-attach from lending one twice: the daemon forgets
    // every slot when the panel changes hands and offers them all again, and these are
    // the ones that are not its to offer.
    let mut lent = lent;
    loop {
        let waited = Instant::now();
        // **Take what the daemon has said before going to sleep on the recorder.** A
        // freed slot only exists once this thread has read the reply carrying it, and the
        // recorder cannot record without one.
        if !lend(panel.released(false)?, &mut lent, back) {
            break;
        }
        let frame = loop {
            match mail.take_until(poll) {
                Waited::Frame(frame) => break frame,
                Waited::Quit => return Ok(()),
                Waited::Nothing => {
                    if !lend(panel.released(false)?, &mut lent, back) {
                        return Ok(());
                    }
                }
            }
        };
        let got = Instant::now();
        panel.submit(frame.slot)?;
        let submitted = Instant::now();
        panel.wait(frame.slot)?;
        let fenced = Instant::now();
        // The fence is signalled: the staging buffer and the uniform block this frame
        // named are the recorder's to write again. This goes back before the present, so
        // the next pack is not waiting on a vsync.
        if back.send(FromPresenter::Retired).is_err() {
            break;
        }
        panel.present(frame.slot)?;
        let shown = Instant::now();
        lent.retain(|s| *s != frame.slot);
        stats.add(&stats.lent_sum, lent.len() as u64);
        // Everything that has arrived, always; and only a recorder with nothing left to
        // draw into is worth *waiting* on the daemon for. A shutting-down recorder is not
        // waiting for anything, and the daemon may be the thing that went away.
        let block = lent.is_empty() && !mail.quitting();
        let free = panel.released(block)?;
        let slotted = Instant::now();

        stats.add(&stats.idle_ns, (got - waited).as_nanos() as u64);
        stats.add(&stats.submit_ns, (submitted - got).as_nanos() as u64);
        stats.add(&stats.fence_ns, (fenced - submitted).as_nanos() as u64);
        stats.add(&stats.show_ns, (shown - fenced).as_nanos() as u64);
        stats.add(&stats.slots_ns, (slotted - shown).as_nanos() as u64);
        let work = (fenced - got).as_nanos() as u64;
        if frame.redrew {
            stats.add(&stats.redrawn, 1);
            stats.add(&stats.redraw_ns, work);
        } else {
            stats.add(&stats.reshow_ns, work);
        }
        if block {
            stats.add(&stats.starved, 1);
        }
        stats.add(&stats.presented, 1);

        if !lend(free, &mut lent, back) {
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
    use std::time::Duration;

    /// The tests' poll interval. Short enough that a timeout costs nothing and long
    /// enough that a loop with nothing to do is not a spin.
    const POLL: Duration = Duration::from_micros(200);

    fn frame(slot: usize) -> Frame {
        Frame {
            slot,
            redrew: true,
            version: None,
        }
    }

    /// A frame recorded at a known world version.
    fn at(slot: usize, version: u64) -> Frame {
        Frame {
            slot,
            redrew: true,
            version: Some(version),
        }
    }

    /// **The work the main thread was doing twice.** A frame is already waiting and the
    /// world has not moved since it was recorded, so re-recording it would build the same
    /// command buffer out of the same texture. The board's main thread is the panel's
    /// ceiling now; this is one of the things it was spending itself on.
    #[test]
    fn a_waiting_frame_that_still_shows_this_world_is_left_alone() {
        assert_eq!(
            next_frame(Some(at(1, 7)), Some(7), Some(2), 1, 2),
            Next::Keep
        );
    }

    /// Once the world has moved the waiting frame is stale, and it is replaced in its own
    /// slot — the one it gives back — so the newest world is what goes up.
    #[test]
    fn a_waiting_frame_whose_world_has_moved_is_replaced_in_its_own_slot() {
        assert_eq!(
            next_frame(Some(at(1, 7)), Some(8), Some(2), 1, 2),
            Next::Replace { slot: 1 },
            "its own slot, not the free one: nothing else has touched it"
        );
    }

    /// A renderer that does not version its content is never kept — the old behaviour,
    /// unchanged, for anything but the voxel strip.
    #[test]
    fn a_renderer_that_cannot_say_what_it_holds_is_always_replaced() {
        assert_eq!(
            next_frame(Some(frame(1)), None, Some(2), 1, 2),
            Next::Replace { slot: 1 }
        );
        assert_eq!(
            next_frame(Some(at(1, 7)), None, Some(2), 1, 2),
            Next::Replace { slot: 1 }
        );
    }

    /// With nothing waiting it records into the slot the presenter lent it, and with no
    /// slot — or a ring already full — the frame is dropped rather than waited for.
    #[test]
    fn nothing_waiting_records_and_nothing_free_drops() {
        assert_eq!(
            next_frame(None, Some(7), Some(2), 0, 2),
            Next::Record { slot: 2 }
        );
        assert_eq!(next_frame(None, Some(7), None, 0, 2), Next::Drop);
        assert_eq!(next_frame(None, Some(7), Some(2), 2, 2), Next::Drop);
        assert_eq!(
            next_frame(Some(at(1, 7)), Some(8), Some(2), 3, 2),
            Next::Drop,
            "even taking the stale one back would leave the ring full"
        );
    }

    /// A panel that records what it was asked to do and answers from a script.
    #[derive(Default)]
    struct FakePanel {
        log: Vec<String>,
        /// What `released` hands back, one call at a time.
        releases: std::collections::VecDeque<Vec<usize>>,
        fail_on_submit: Option<usize>,
    }

    impl Panel for FakePanel {
        fn submit(&mut self, slot: usize) -> Result<()> {
            self.log.push(format!("submit {slot}"));
            if self.fail_on_submit == Some(slot) {
                anyhow::bail!("the device is gone");
            }
            Ok(())
        }
        fn wait(&mut self, slot: usize) -> Result<()> {
            self.log.push(format!("wait {slot}"));
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
        assert_eq!(mail.post(frame(0)), None, "nothing was waiting");
        assert_eq!(mail.post(frame(1)), Some(frame(0)), "the first comes back");
        assert_eq!(mail.post(frame(2)), Some(frame(1)));
        assert_eq!(mail.take(), Some(frame(2)), "only the newest is presented");
    }

    /// The recorder takes its own frame back before recording a fresher one, so the frame
    /// it gives up is always the last one recorded — which is the only one the renderer's
    /// ring can give back.
    #[test]
    fn a_frame_the_presenter_has_not_started_is_reclaimed() {
        let mail = Mailbox::new();
        assert_eq!(mail.reclaim(), None, "nothing to reclaim");
        mail.post(frame(2));
        assert_eq!(mail.reclaim(), Some(frame(2)));
        assert_eq!(mail.reclaim(), None, "and only once");
    }

    /// Neither posting nor reclaiming ever waits for the presenter: both are a lock and
    /// an `Option`. (If either blocked, this test would hang rather than fail.)
    #[test]
    fn the_recorder_never_waits_on_the_presenter() {
        let mail = Mailbox::new();
        for i in 0..1000 {
            mail.post(frame(i % 3));
            mail.reclaim();
        }
        assert_eq!(mail.take_now(), None);
    }

    /// One frame in flight: submit, wait, say it has retired, present, then look for the
    /// next free slot — in that order, because the recorder should get its staging buffer
    /// back without waiting for a vsync.
    #[test]
    fn a_frame_is_submitted_retired_and_then_presented() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let stats = PresentStats::default();
        let mut panel = FakePanel {
            releases: [vec![]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(frame(0));
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &stats, vec![0, 1, 2], POLL).unwrap();

        assert_eq!(
            panel.log,
            vec![
                // Every pass reads what the daemon has already said, before anything else.
                "released block=false",
                "submit 0",
                "wait 0",
                "present 0",
                // The recorder still holds 1 and 2, so nothing sleeps on a reply.
                "released block=false",
                "released block=false",
            ]
        );
        assert_eq!(stats.presented(), 1);
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
        let stats = PresentStats::default();
        let mut panel = FakePanel {
            // The re-attach: the daemon has forgotten everything and offers it all.
            releases: [vec![], vec![0, 1, 2]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(frame(1));
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &stats, vec![0, 1, 2], POLL).unwrap();

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
        let stats = PresentStats::default();
        let mut panel = FakePanel::default();
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &stats, vec![0], POLL).unwrap();
        assert_eq!(stats.presented(), 0, "nothing went up");
        assert_eq!(panel.log, vec!["released block=false"], "{:?}", panel.log);
        drop(rx);
    }

    /// A real shutdown races: the recorder posts a frame and quits while the presenter is
    /// still working. The thread drains the one frame it was handed and then stops, so a
    /// join never waits on a frame and never loses one silently.
    #[test]
    fn a_frame_posted_as_the_run_ends_is_still_presented_and_then_the_thread_stops() {
        let mail = std::sync::Arc::new(Mailbox::new());
        let (tx, rx) = channel();
        let stats = std::sync::Arc::new(PresentStats::default());
        let (m, s) = (mail.clone(), stats.clone());
        let thread = std::thread::spawn(move || {
            let mut panel = FakePanel {
                releases: [vec![0]].into_iter().collect(),
                ..FakePanel::default()
            };
            present_loop(&mut panel, &m, &tx, &s, vec![0, 1], POLL).unwrap();
            panel.log
        });
        mail.post(frame(1));
        mail.quit();
        // The thread ran: it says the frame's resources are free again.
        assert_eq!(rx.recv().unwrap(), FromPresenter::Retired);
        let log = thread.join().expect("the presenter thread ends");
        assert_eq!(stats.presented(), 1, "the last frame went up");
        assert!(log.contains(&"present 1".to_string()), "{log:?}");
    }

    /// A panel that fails ends the loop with its error, which the recorder turns into the
    /// run's failure — the same end a synchronous present had.
    #[test]
    fn a_failing_panel_ends_the_loop() {
        let mail = Mailbox::new();
        let (tx, _rx) = channel();
        let stats = PresentStats::default();
        let mut panel = FakePanel {
            fail_on_submit: Some(0),
            ..FakePanel::default()
        };
        mail.post(frame(0));
        mail.quit();
        let e = present_loop(&mut panel, &mail, &tx, &stats, vec![0], POLL).expect_err("it failed");
        assert!(format!("{e:#}").contains("the device is gone"), "{e:#}");
    }

    /// A recorder that has gone away (its receiver dropped) is a shutdown, not a failure:
    /// the frame it left behind is submitted and the thread ends without an error.
    #[test]
    fn a_gone_recorder_ends_the_loop_quietly() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        drop(rx);
        let stats = PresentStats::default();
        let mut panel = FakePanel::default();
        mail.post(frame(0));
        present_loop(&mut panel, &mail, &tx, &stats, vec![0], POLL).expect("no error");
        assert_eq!(
            panel.log,
            vec!["released block=false", "submit 0", "wait 0"],
            "it stopped at the first send"
        );
    }

    /// **The stall the board found.** The daemon reports a freed slot only in a reply,
    /// and only when a flip completes. A presenter asleep waiting for a frame is not
    /// reading those replies — and the recorder, holding no slot, cannot record the frame
    /// that would wake it. So the wait is bounded: with nothing posted at all, the slot
    /// the daemon frees still reaches the recorder.
    #[test]
    fn a_slot_freed_while_nothing_is_posted_still_reaches_the_recorder() {
        let mail = std::sync::Arc::new(Mailbox::new());
        let (tx, rx) = channel();
        let stats = std::sync::Arc::new(PresentStats::default());
        let (m, s) = (mail.clone(), stats.clone());
        let thread = std::thread::spawn(move || {
            let mut panel = FakePanel {
                // Nothing on the first look; the flip completes on the second.
                releases: [vec![], vec![2]].into_iter().collect(),
                ..FakePanel::default()
            };
            // The recorder holds nothing: exactly the state the board was stuck in.
            present_loop(&mut panel, &m, &tx, &s, Vec::new(), POLL).unwrap();
            panel.log
        });
        // No frame is ever posted, and the slot still arrives.
        assert_eq!(rx.recv().unwrap(), FromPresenter::Free(2));
        mail.quit();
        let log = thread.join().expect("the presenter thread ends");
        assert!(
            log.iter().all(|l| l.starts_with("released")),
            "it read the socket and did nothing else: {log:?}"
        );
        assert_eq!(stats.presented(), 0);
    }

    /// A lent slot is the recorder's for as short a time as it can be: the moment a frame
    /// is presented its slot leaves the lent set, so the next reply that frees it hands it
    /// straight back. The average number in hand is reported, because that depth is what
    /// says whether the panel's rate is a slot supply problem.
    #[test]
    fn a_presented_slot_leaves_the_lent_set_and_comes_back_on_the_next_reply() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let stats = PresentStats::default();
        let mut panel = FakePanel {
            // Before the frame: nothing. After presenting it: the daemon frees it.
            releases: [vec![], vec![0], vec![]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(frame(0));
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &stats, vec![0], POLL).unwrap();

        let free: Vec<usize> = rx
            .try_iter()
            .filter_map(|m| match m {
                FromPresenter::Free(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(free, vec![0], "handed back as soon as the daemon freed it");
        let now = stats.snapshot();
        assert_eq!(now.lent_sum, 0, "the recorder held nothing at that present");
        assert_eq!(
            now.starved, 0,
            "a recorder that is shutting down is never waited on"
        );
        assert!(now.line(1.0, "a fake panel").contains("0.0 slots in hand"));
    }

    /// **The point of the report.** A thread that turns out to be the panel's ceiling has
    /// to be able to say which part of itself is the ceiling: the phases are timed apart,
    /// and a frame that redrew the world is timed apart from one that only put an
    /// already-drawn raster onto a new slot.
    #[test]
    fn the_report_splits_the_phases_and_the_two_kinds_of_frame() {
        let mail = Mailbox::new();
        let (tx, rx) = channel();
        let stats = PresentStats::default();
        let mut panel = FakePanel {
            releases: [vec![], vec![]].into_iter().collect(),
            ..FakePanel::default()
        };
        mail.post(Frame {
            slot: 0,
            redrew: true,
            version: None,
        });
        mail.quit();
        present_loop(&mut panel, &mail, &tx, &stats, vec![0, 1], POLL).unwrap();
        mail.post(Frame {
            slot: 1,
            redrew: false,
            version: None,
        });
        present_loop(&mut panel, &mail, &tx, &stats, vec![1], POLL).unwrap();
        drop(rx);

        let now = stats.snapshot();
        assert_eq!(now.presented, 2);
        assert_eq!(now.redrawn, 1, "one of the two redrew the world");
        let line = now.line(60.0, "a fake panel");
        assert!(line.contains("2 shown (0.0/s), 1 redrew"), "{line}");
        assert!(line.contains("re-present"), "{line}");
        assert!(line.contains("on a fake panel"), "{line}");

        // An interval is the difference of two snapshots, so each report is about its own
        // minute rather than about the whole run.
        let d = now.since(&PresentSample {
            presented: 1,
            redrawn: 1,
            ..PresentSample::default()
        });
        assert_eq!(d.presented, 1);
        assert_eq!(d.redrawn, 0);
    }
}

#[cfg(test)]
mod report {
    use super::*;

    /// The line as the board will print it, built from the numbers the board actually
    /// reported (1197 presented in 60 s, a thread at 81 % of a core). Forty milliseconds
    /// of *running* per present is the whole story of the panel's rate, and the line has
    /// to make it readable at a glance rather than inferred from `top`.
    #[test]
    fn the_report_reads_as_one_line() {
        let ms = |n: f64, count: u64| (n * 1e6) as u64 * count;
        let sample = PresentSample {
            presented: 1197,
            redrawn: 1180,
            idle_ns: ms(0.4, 1197),
            submit_ns: ms(2.1, 1197),
            fence_ns: ms(44.9, 1197),
            show_ns: ms(0.3, 1197),
            slots_ns: ms(1.2, 1197),
            redraw_ns: ms(47.6, 1180),
            reshow_ns: ms(8.1, 17),
            starved: 12,
            lent_sum: 2 * 1197,
            held: 402,
            skipped: 1853,
            refused_packs: 0,
        };
        let line = sample.line(60.0, "Mali-G610 (Panfrost)");
        println!("{line}");
        assert!(line.contains("1197 shown (19.9/s), 1180 redrew"), "{line}");
        assert!(line.contains("fence 44.9"), "{line}");
        assert!(
            line.contains("redraw 47.6 ms vs re-present 8.1 ms (17)"),
            "{line}"
        );
        assert!(line.contains("1853 frames had no slot"), "{line}");
        assert!(line.contains("2.0 slots in hand"), "{line}");
        assert!(line.contains("402 already current"), "{line}");
    }
}

#[cfg(test)]
impl Mailbox {
    /// The waiting frame without waiting for one, for the tests.
    fn take_now(&self) -> Option<Frame> {
        self.lock().frame.take()
    }
}
