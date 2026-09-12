//! The `demo` loop: one clock, one scene set, one `Canvas::encode` per rendered frame,
//! and whichever sink is active. The persistent `run` loop lives in [`crate::runner`].
//!
//! This module also owns the process-wide SIGINT handler. [`run`] installs it once,
//! before either loop starts, and both loops read the flag it sets once per iteration:
//! the first Ctrl-C is a clean stop (the `run` loop's final snapshot, the sinks'
//! `finish`), a second one exits immediately with the shell's 128+SIGINT status.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use cube_proto::Frame;
use cubarium_render::Canvas;
use cubarium_surface::PixelImage;

use crate::clock::{Clock, Step};
use crate::cli::{Command, Demo, SinkArg};
use crate::scene::{SceneKind, Scenes, render};
use crate::sink::{FrameSink, PngSink, PreviewSink, ShimSink, WebSink};

/// The status a shell reports for a process killed by SIGINT.
const SIGINT_EXIT: i32 = 130;

/// Install the SIGINT handler and return the flag it sets. A failure to install one is
/// reported and the command runs anyway: losing the clean stop is not worth losing the
/// run. Calling this twice in one process is what `ctrlc` refuses, so [`run`] is the
/// only caller.
fn install_interrupt_handler() -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    // `ctrlc` runs this on its own thread, not in the signal handler itself, so the
    // message and the exit are both safe here.
    let installed = ctrlc::set_handler(move || {
        if flag.swap(true, Ordering::SeqCst) {
            eprintln!("cubarium: interrupted again; exiting without a final snapshot");
            std::process::exit(SIGINT_EXIT);
        }
        eprintln!("cubarium: interrupted; stopping cleanly (Ctrl-C again to exit now)");
    });
    if let Err(e) = installed {
        eprintln!("cubarium: no Ctrl-C handler ({e}); an interrupt will not stop cleanly");
    }
    stop
}

/// Run a parsed command.
pub fn run(command: Command) -> Result<()> {
    let stop = install_interrupt_handler();
    match command {
        Command::Demo(demo) => {
            demo.validate()?;
            run_demo(&demo, &stop)
        }
        Command::Run(world) => crate::runner::run_world_until(&world, &stop).map(|_| ()),
    }
}

fn run_demo(demo: &Demo, stop: &AtomicBool) -> Result<()> {
    let kind: SceneKind = demo.scene.into();
    let mut scenes = Scenes::new(kind, demo.seed);

    let mut sink: Box<dyn FrameSink> = match demo.sink {
        SinkArg::Preview => Box::new(PreviewSink::new(demo.scale, &demo.out)?),
        SinkArg::Shim => Box::new(ShimSink::new(demo.addr.clone())),
        SinkArg::Png => Box::new(PngSink::new(&demo.out, demo.every)?),
        SinkArg::Web => Box::new(WebSink::new(demo.web_port)?),
    };

    let limit =
        (demo.seconds > 0.0).then(|| Duration::from_secs_f64(demo.seconds));
    let stats = drive(&mut scenes, sink.as_mut(), limit, demo.fps, stop)?;
    sink.finish()?;

    eprintln!(
        "cubarium: {} ticks, {} frames in {:.2} s ({:.1} fps)",
        stats.ticks,
        stats.frames,
        stats.elapsed.as_secs_f64(),
        stats.frames as f64 / stats.elapsed.as_secs_f64().max(1e-9),
    );
    Ok(())
}

/// What one run produced.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunStats {
    pub ticks: u64,
    pub frames: u64,
    pub dropped_frames: u64,
    pub pauses: u64,
    pub elapsed: Duration,
}

/// Drive scenes into a sink at `fps` until the time limit, the sink asking to stop, or
/// `stop` being set from another thread (the SIGINT handler, or a test). The simulation
/// runs at 20 Hz whatever `fps` is; each frame is drawn at the clock's interpolation
/// fraction of the tick in progress.
pub fn drive(
    scenes: &mut Scenes,
    sink: &mut dyn FrameSink,
    limit: Option<Duration>,
    fps: u32,
    stop: &AtomicBool,
) -> Result<RunStats> {
    let mut canvas = Canvas::new();
    let mut scratch: Vec<PixelImage> = Vec::new();
    let mut frame = Frame::black();
    let start = Instant::now();
    let mut clock = Clock::with_fps(start, fps);
    let mut stats = RunStats::default();

    loop {
        let now = Instant::now();
        if let Some(l) = limit
            && clock.elapsed(now) >= l
        {
            break;
        }
        if sink.should_quit() || stop.load(Ordering::Relaxed) {
            break;
        }

        match clock.next_step(now) {
            Step::Tick => {
                scenes.tick();
                stats.ticks += 1;
            }
            Step::Render { f } => {
                // The render view is a cloned snapshot of the last completed tick; `f`
                // interpolates each body along the path it traveled during that tick.
                let view = scenes.view();
                render(&view, f, &mut canvas, &mut scratch);
                // Exactly one encode per rendered frame; the identical bytes go to the
                // active sink.
                canvas.encode(&mut frame);
                sink.submit(&frame)?;
                stats.frames += 1;
            }
            Step::Sleep(d) => std::thread::sleep(d),
            Step::Lagged { behind, log } => {
                stats.dropped_frames += 1;
                if log {
                    eprintln!("cubarium: behind by {:.0} ms; dropping render work", behind.as_secs_f64() * 1e3);
                }
            }
            Step::Paused { gap } => {
                stats.pauses += 1;
                eprintln!("cubarium: clock re-based after a {:.1} s pause", gap.as_secs_f64());
            }
        }
    }

    stats.elapsed = clock.elapsed(Instant::now());
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;

    /// A sink that keeps every frame it is given.
    #[derive(Default)]
    struct Recorder {
        frames: Vec<Frame>,
    }

    impl FrameSink for Recorder {
        fn submit(&mut self, frame: &Frame) -> Result<()> {
            self.frames.push(frame.clone());
            Ok(())
        }
    }

    #[test]
    fn a_short_run_ticks_renders_and_lights_more_than_one_face() {
        let mut scenes = Scenes::new(SceneKind::All, 1);
        let mut rec = Recorder::default();
        let stop = AtomicBool::new(false);
        let stats =
            drive(&mut scenes, &mut rec, Some(Duration::from_millis(600)), 60, &stop).unwrap();
        assert!(stats.ticks >= 8, "ticks {}", stats.ticks);
        assert!(stats.frames >= 12, "frames {}", stats.frames);
        assert_eq!(rec.frames.len() as u64, stats.frames);

        let last = rec.frames.last().unwrap();
        let mut faces = std::collections::HashSet::new();
        for face in Face::ALL {
            if last.face(face).iter().any(|&b| b != 0) {
                faces.insert(face);
            }
        }
        assert!(faces.len() > 1, "a frame must light more than one face: {faces:?}");
    }

    #[test]
    fn a_sink_that_asks_to_quit_stops_the_loop() {
        struct Once(u32);
        impl FrameSink for Once {
            fn submit(&mut self, _f: &Frame) -> Result<()> {
                self.0 += 1;
                Ok(())
            }
            fn should_quit(&mut self) -> bool {
                self.0 >= 3
            }
        }
        let mut scenes = Scenes::new(SceneKind::Body, 1);
        let mut sink = Once(0);
        let stats = drive(&mut scenes, &mut sink, None, 60, &AtomicBool::new(false)).unwrap();
        assert_eq!(stats.frames, 3);
    }

    #[test]
    fn a_stop_flag_set_from_another_thread_ends_the_demo_loop() {
        struct Counter(u64);
        impl FrameSink for Counter {
            fn submit(&mut self, _f: &Frame) -> Result<()> {
                self.0 += 1;
                Ok(())
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let setter = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::SeqCst);
        });
        let mut scenes = Scenes::new(SceneKind::Body, 1);
        let mut sink = Counter(0);
        // No time limit: only the flag can end this loop.
        let stats = drive(&mut scenes, &mut sink, None, 60, &stop).unwrap();
        setter.join().unwrap();
        assert!(stats.frames > 0, "the loop must have rendered before it stopped");
        assert!(
            stats.elapsed < Duration::from_secs(5),
            "the loop must stop promptly: {:?}",
            stats.elapsed
        );
    }
}
