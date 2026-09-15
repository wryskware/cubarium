//! Output sinks. Every sink consumes the identical `Frame` bytes produced by one
//! `Canvas::encode` per rendered frame.

pub mod fanout;
pub mod png;
pub mod preview;
pub mod shim;
pub mod web;

use anyhow::Result;
use cube_proto::Frame;

pub use fanout::FanOutSink;
pub use png::PngSink;
pub use preview::PreviewSink;
pub use shim::ShimSink;
pub use web::WebSink;

/// Where rendered frames go.
pub trait FrameSink {
    /// Consume one encoded frame. Must not block on I/O.
    fn submit(&mut self, frame: &Frame) -> Result<()>;

    /// The world's tick, after each completed tick. Sinks that only carry pixels ignore
    /// it; the web sink reports it so a viewer can name the world it is watching. It is
    /// observation only — nothing a sink does here can reach the world.
    fn observe_tick(&mut self, _tick: u64) {}

    /// The living population and how many of them run a recurrent policy, after each
    /// completed tick. Observation only, on the same one-way rule as [`Self::observe_tick`];
    /// the web sink reports both at `/status` so a seeded run can be checked from outside.
    fn observe_counts(&mut self, _population: usize, _neural: usize) {}

    /// True once the sink wants the host to stop (the preview window was closed).
    fn should_quit(&mut self) -> bool {
        false
    }

    /// Called once on a clean stop.
    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}
