//! Output sinks. Every sink consumes the identical `Frame` bytes produced by one
//! `Canvas::encode` per rendered frame.

pub mod png;
pub mod preview;
pub mod shim;
pub mod web;

use anyhow::Result;
use cube_proto::Frame;

pub use png::PngSink;
pub use preview::PreviewSink;
pub use shim::ShimSink;
pub use web::WebSink;

/// Where rendered frames go.
pub trait FrameSink {
    /// Consume one encoded frame. Must not block on I/O.
    fn submit(&mut self, frame: &Frame) -> Result<()>;

    /// True once the sink wants the host to stop (the preview window was closed).
    fn should_quit(&mut self) -> bool {
        false
    }

    /// Called once on a clean stop.
    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}
