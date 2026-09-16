//! Output sinks. Every sink consumes the identical pixels produced by one canvas encode
//! per rendered frame — a `Frame` on a cube world, a `Raster` on a ring world.

pub mod fanout;
pub mod gpu;
pub mod png;
pub mod preview;
pub mod shim;
pub mod web;

use anyhow::Result;
use cube_proto::{FACE_SIZE, Frame, Raster};
use cubarium_core::hunter::{HunterEvent, HunterView};
use cubarium_core::view::RenderView;
use cubarium_surface::{Scale, Topology};

pub use fanout::FanOutSink;
pub use gpu::{GpuSink, GpuSinkOptions, GpuTargetKind};
pub use png::PngSink;
pub use preview::PreviewSink;
pub use shim::ShimSink;
pub use web::WebSink;

/// One rendered frame, in the shape the world has.
///
/// The enum rather than a generic parameter is deliberate ([the ring-world
/// plan](../../../../design/flat-world-plan-2026-09-16.md) §3): [`FrameSink`] must stay
/// object-safe, because [`FanOutSink`] holds `Vec<Box<dyn FrameSink>>` and because the
/// host picks its sink at runtime from `--sink`. A generic would monomorphise the whole
/// host per topology and could not be boxed at all.
///
/// A sink is handed whichever variant the world produces and is never asked to convert
/// between them: a cube frame and a ring raster are different pictures of different
/// worlds, not two encodings of one.
#[derive(Clone, Copy)]
pub enum Output<'a> {
    /// A cube world: five 64×64 charts, `cube_proto` wire format 0.
    Cube(&'a Frame),
    /// A ring world: one `w×h` image, `cube_proto` wire format 2 (raster strips).
    Ring(&'a Raster),
}

impl<'a> Output<'a> {
    /// `"cube"` or `"ring"` — the same word [`WorldShape::name`] uses and `/status`
    /// reports.
    pub fn name(self) -> &'static str {
        match self {
            Output::Cube(_) => "cube",
            Output::Ring(_) => "ring",
        }
    }

    /// The image's pixel size: one chart for a cube (64×64), the whole raster for a ring.
    pub fn size(self) -> (u16, u16) {
        match self {
            Output::Cube(_) => (FACE_SIZE as u16, FACE_SIZE as u16),
            Output::Ring(r) => r.size(),
        }
    }

    /// The encoded pixel bytes, tightly packed RGB8. `FRAME_BYTES` for a cube; `w·h·3`
    /// for a ring.
    pub fn bytes(self) -> &'a [u8] {
        match self {
            Output::Cube(f) => f.as_bytes().as_slice(),
            Output::Ring(r) => r.as_bytes(),
        }
    }

    /// The cube frame, or `None` on a ring. For the sinks that only ever knew the cube.
    pub fn frame(self) -> Option<&'a Frame> {
        match self {
            Output::Cube(f) => Some(f),
            Output::Ring(_) => None,
        }
    }
}

impl std::fmt::Debug for Output<'_> {
    /// The shape and the size, never the pixels: a frame is 61,440 bytes and a raster can
    /// be megabytes, and neither belongs in an error message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (w, h) = self.size();
        write!(f, "Output::{}({w}x{h})", self.name())
    }
}

/// The shape of the world a sink is showing, fixed for the life of the run.
///
/// A sink needs this *before* the first frame — the viewer's `/status` answers a page
/// that has not polled `/frame` yet, and the preview window has to refuse a ring at
/// construction rather than at the first `submit`, which is after the window is already
/// on screen. So it is carried beside the frames, not derived from them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldShape {
    pub topology: Topology,
    pub scale: Scale,
}

impl WorldShape {
    /// The cube at `S = 1`: what every sink showed before the ring existed, and the
    /// default for the entry points that do not name a shape.
    pub const CUBE: WorldShape = WorldShape { topology: Topology::Cube, scale: Scale::ONE };

    pub fn new(topology: Topology, scale: Scale) -> WorldShape {
        WorldShape { topology, scale }
    }

    /// `"cube"` or `"ring"`.
    pub fn name(self) -> &'static str {
        match self.topology {
            Topology::Cube => "cube",
            Topology::Ring { .. } => "ring",
        }
    }

    pub fn is_ring(self) -> bool {
        matches!(self.topology, Topology::Ring { .. })
    }

    /// The chart's pixel size: 64×64 for a cube, `w×h` for a ring. This is what
    /// `/status` reports as `w` and `h`, and — for a ring — the size of the raster every
    /// frame carries.
    pub fn chart_size(self) -> (u16, u16) {
        match self.topology {
            Topology::Cube => (FACE_SIZE as u16, FACE_SIZE as u16),
            Topology::Ring { w, h } => (w, h),
        }
    }

    /// A black raster of this world's size, or `None` for a cube, which encodes into a
    /// [`Frame`] instead.
    pub fn raster(self) -> Option<Raster> {
        match self.topology {
            Topology::Cube => None,
            Topology::Ring { w, h } => Some(Raster::black(w, h)),
        }
    }
}

impl Default for WorldShape {
    fn default() -> WorldShape {
        WorldShape::CUBE
    }
}

/// Where rendered frames go.
pub trait FrameSink {
    /// Consume one rendered frame. Must not block on I/O.
    ///
    /// The trait is object-safe and must stay so: [`FanOutSink`] boxes its children.
    fn submit(&mut self, out: Output<'_>) -> Result<()>;

    /// The world's tick, after each completed tick. Sinks that only carry pixels ignore
    /// it; the web sink reports it so a viewer can name the world it is watching. It is
    /// observation only — nothing a sink does here can reach the world.
    fn observe_tick(&mut self, _tick: u64) {}

    /// The living population and how many of them run a recurrent policy, after each
    /// completed tick. Observation only, on the same one-way rule as [`Self::observe_tick`];
    /// the web sink reports both at `/status` so a seeded run can be checked from outside.
    fn observe_counts(&mut self, _population: usize, _neural: usize) {}

    /// The world's own view once per completed tick, with the hunter membership and the
    /// hunter events it committed — exactly what the host hands its presenter at the same
    /// instant, and on the same one-way rule: nothing a sink does here can reach the world.
    ///
    /// A sink that draws the world itself rather than consuming the host's pixels needs
    /// this, because a `RenderView` alone does not carry the state a picture has: paced
    /// growth, column heights, body cross-fades, hunter phases. Default no-op, so every
    /// pixel sink is unaffected.
    fn observe_world(
        &mut self,
        _view: &RenderView,
        _hunters: &[HunterView],
        _events: &[HunterEvent],
    ) {
    }

    /// One rendered frame's view, at presentation `seconds` and tick fraction `f` — the
    /// same two numbers [`crate::art_present::ArtPresenter::draw`] is called with, at the
    /// same point in the host's loop.
    ///
    /// The pair with [`Self::observe_world`] is the presenter's own two-method contract
    /// (observe once per tick, draw once per frame), offered to a sink. Default no-op.
    fn observe_view(&mut self, _view: &RenderView, _seconds: f64, _f: f64) -> Result<()> {
        Ok(())
    }

    /// Whether this sink needs the pixels [`Self::submit`] carries.
    ///
    /// `false` lets the host skip the canvas entirely — no `draw`, no encode, no submit —
    /// which is the whole point of a sink that renders the world itself: a GPU sink that
    /// still paid for a 23 ms CPU rasterisation it threw away would be slower than the
    /// CPU path, not faster. A fan-out wants pixels if **any** child does.
    fn wants_pixels(&self) -> bool {
        true
    }

    /// True once the sink wants the host to stop (the preview window was closed).
    fn should_quit(&mut self) -> bool {
        false
    }

    /// Called once on a clean stop.
    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}
