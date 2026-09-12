//! The `demo` loop: one clock, one scene set, one `Canvas::encode` per rendered frame,
//! and whichever sink is active. The persistent `run` loop lives in [`crate::runner`].

use std::time::{Duration, Instant};

use anyhow::Result;
use cube_proto::Frame;
use cubarium_render::Canvas;
use cubarium_surface::PixelImage;

use crate::clock::{Clock, Step};
use crate::cli::{Command, Demo, SinkArg};
use crate::scene::{SceneKind, Scenes, render};
use crate::sink::{FrameSink, PngSink, PreviewSink, ShimSink};

/// Run a parsed command.
pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Demo(demo) => {
            demo.validate()?;
            run_demo(&demo)
        }
        Command::Run(world) => crate::runner::run_world(&world).map(|_| ()),
    }
}

fn run_demo(demo: &Demo) -> Result<()> {
    let kind: SceneKind = demo.scene.into();
    let mut scenes = Scenes::new(kind, demo.seed);

    let mut sink: Box<dyn FrameSink> = match demo.sink {
        SinkArg::Preview => Box::new(PreviewSink::new(demo.scale, &demo.out)?),
        SinkArg::Shim => Box::new(ShimSink::new(demo.addr.clone())),
        SinkArg::Png => Box::new(PngSink::new(&demo.out, demo.every)?),
    };

    let limit =
        (demo.seconds > 0.0).then(|| Duration::from_secs_f64(demo.seconds));
    let stats = drive(&mut scenes, sink.as_mut(), limit)?;
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

/// Drive scenes into a sink until the time limit or the sink asks to stop.
pub fn drive(
    scenes: &mut Scenes,
    sink: &mut dyn FrameSink,
    limit: Option<Duration>,
) -> Result<RunStats> {
    let mut canvas = Canvas::new();
    let mut scratch: Vec<PixelImage> = Vec::new();
    let mut frame = Frame::black();
    let start = Instant::now();
    let mut clock = Clock::new(start);
    let mut stats = RunStats::default();

    loop {
        let now = Instant::now();
        if let Some(l) = limit
            && clock.elapsed(now) >= l
        {
            break;
        }
        if sink.should_quit() {
            break;
        }

        match clock.next_step(now) {
            Step::Tick => {
                scenes.tick();
                stats.ticks += 1;
            }
            Step::Render => {
                // The render view is a cloned snapshot of the last completed tick.
                let view = scenes.view();
                render(&view, &mut canvas, &mut scratch);
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
        let stats = drive(&mut scenes, &mut rec, Some(Duration::from_millis(600))).unwrap();
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
        let stats = drive(&mut scenes, &mut sink, None).unwrap();
        assert_eq!(stats.frames, 3);
    }
}
