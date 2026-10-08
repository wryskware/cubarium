//! **`WorldRenderer`**: the Rust renderer (`cubarium-gpu` and its pack, D-0005) drawing a
//! session's world into frames the world view shows. A thin class over
//! [`cubarium::sink::gpu::voxel::hosted::Hosted`]: the renderer runs on the session's loop
//! and a presenter thread of its own, and Godot takes the newest frame when it likes.

use std::path::PathBuf;

use cubarium::sink::gpu::voxel::hosted::{Hosted, HostedOptions};
use cubarium::voxel::figure::{FigureMotion, Movement};
use godot::classes::RenderingServer;
use godot::prelude::*;

use crate::CubariumSession;

/// How far a highlighted animal leans toward white: the mesh view's overlay (white at 0.4).
const HIGHLIGHT_MIX: f32 = 0.4;

/// The world's renderer for one view. `start` it, then `attach` a session or
/// `show_candidate` one of its candidates, and `take_frame` each frame.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct WorldRenderer {
    hosted: Option<Hosted>,
    /// The session this renderer is hooked into, so `detach` and a drop unhook it.
    attached: Option<Gd<CubariumSession>>,
}

#[godot_api]
impl WorldRenderer {
    /// Start the renderer: `config` is the presentation config (a `cubarium voxel` config
    /// file), `root` what its relative asset paths are relative to (the repository), `screen`
    /// the size it draws for (`px_per_voxel = "auto"` covers it), `fps` the most frames a
    /// second. Returns "" or why not.
    #[func]
    fn start(&mut self, config: GString, root: GString, screen: Vector2i, fps: f64) -> GString {
        // Draw on the GPU Godot draws on, so a frame never crosses between GPUs on its way
        // to the screen (`CUBARIUM_GPU` still overrides).
        let adapter = RenderingServer::singleton().get_video_adapter_name().to_string();
        if !adapter.is_empty() {
            cubarium_gpu::vk::prefer_device(&adapter);
        }
        let options = HostedOptions {
            config: PathBuf::from(config.to_string()),
            root: PathBuf::from(root.to_string()),
            screen: (screen.x.max(1) as u32, screen.y.max(1) as u32),
            fps,
            highlight_mix: HIGHLIGHT_MIX,
        };
        match Hosted::new(options) {
            Ok(h) => {
                crate::teardown::renderer(h.stopper());
                self.hosted = Some(h);
                GString::new()
            }
            Err(e) => GString::from(format!("{e:#}").as_str()),
        }
    }

    /// Draw `session`'s running world (and every world it runs from now on).
    #[func]
    fn attach(&mut self, session: Gd<CubariumSession>) {
        self.detach();
        let Some(h) = &self.hosted else { return };
        session.bind().session.set_hook(Some(h.hook()));
        self.attached = Some(session);
    }

    /// Stop drawing the session; the last frame stays.
    #[func]
    fn detach(&mut self) {
        if let Some(s) = self.attached.take() {
            s.bind().session.set_hook(None);
        }
    }

    /// Draw candidate `index` of `session`, once (its clock stands still), starting now: a
    /// candidate opened large.
    #[func]
    fn show_candidate(&mut self, session: Gd<CubariumSession>, index: i64) {
        self.show_static(session, index, false);
    }

    /// `show_candidate` for a thumbnail: staged in its turn, one at a time across the app,
    /// first asked first drawn (each one's sky plane takes every core but one).
    #[func]
    fn show_thumbnail(&mut self, session: Gd<CubariumSession>, index: i64) {
        self.show_static(session, index, true);
    }

    /// The newest frame since the last take: `rgba` (the raster, sRGB RGBA8 bytes for
    /// `Image.create_from_data`), `width`, `height`, `view_x` (the raster columns it is
    /// turned round the ring), `tick`, `generation`; or null.
    #[func]
    fn take_frame(&self) -> Variant {
        let Some(h) = &self.hosted else { return Variant::nil() };
        let Some(frame) = h.take_frame() else { return Variant::nil() };
        let bytes = PackedByteArray::from(frame.rgba.as_slice());
        let mut d = VarDictionary::new();
        d.set("rgba", &bytes.to_variant());
        d.set("width", &(frame.width as i64).to_variant());
        d.set("height", &(frame.height as i64).to_variant());
        d.set("view_x", &(frame.view_x as i64).to_variant());
        d.set("tick", &(frame.tick as i64).to_variant());
        d.set("generation", &(frame.generation as i64).to_variant());
        h.recycle(frame);
        d.to_variant()
    }

    /// Turn the picture round the ring by `x` raster columns (whole pixels).
    #[func]
    fn set_view_x(&self, x: i64) {
        if let Some(h) = &self.hosted {
            h.set_view_x(x as i32);
        }
    }

    /// Draw animal `id` highlighted (−1: none).
    #[func]
    fn set_highlight(&self, id: i64) {
        if let Some(h) = &self.hosted {
            h.set_highlight(u64::try_from(id).ok());
        }
    }

    /// Draw the animals with `movement` ("smooth" or "stepped") and `subpixel` placement
    /// (smooth only), from the renderer's next frame on, in this world and every one after.
    /// `startup`: a viewer's stored settings as the renderer starts, under what the
    /// environment named (`CUBARIUM_ANIMALS`, `CUBARIUM_ANIMAL_SUBPIXEL`), which keeps its
    /// value; otherwise a viewer's change while it runs, which applies whole. Returns "" or
    /// why not.
    #[func]
    fn set_animal_motion(&self, movement: GString, subpixel: bool, startup: bool) -> GString {
        let name = movement.to_string();
        let Some(movement) = Movement::parse(&name) else {
            return GString::from(format!("unknown movement {name:?}; valid values: smooth, stepped").as_str());
        };
        let Some(h) = &self.hosted else { return GString::from("the renderer has not started") };
        let m = FigureMotion { movement, subpixel };
        if startup {
            h.set_stored_figure_motion(m);
        } else {
            h.set_figure_motion(m);
        }
        GString::new()
    }

    /// The animals' live settings: `models` (false: stamped animals, which they do not
    /// apply to), `movement` and `subpixel` asked for, `drawn_movement` and `drawn_subpixel`
    /// those the newest frame was drawn with (absent before one), and `env_movement`,
    /// `env_subpixel`: whether the environment named them at startup.
    #[func]
    fn animal_motion(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(h) = &self.hosted else { return d };
        let (asked, drawn) = h.figure_motion();
        d.set("models", &asked.is_some().to_variant());
        if let Some(m) = asked {
            d.set("movement", &GString::from(m.movement.name()).to_variant());
            d.set("subpixel", &m.subpixel.to_variant());
        }
        if let Some(m) = drawn {
            d.set("drawn_movement", &GString::from(m.movement.name()).to_variant());
            d.set("drawn_subpixel", &m.subpixel.to_variant());
        }
        let pins = h.figure_env_pins();
        d.set("env_movement", &pins.movement.to_variant());
        d.set("env_subpixel", &pins.subpixel.to_variant());
        d
    }

    /// The animal under world raster pixel `(px, py)`, or nearest within `radius` raster
    /// pixels; −1 for none.
    #[func]
    fn pick(&self, px: i64, py: i64, radius: f64) -> i64 {
        self.hosted
            .as_ref()
            .and_then(|h| h.pick(px as i32, py as i32, radius as f32))
            .map_or(-1, |id| id as i64)
    }

    /// Where animal `id` is drawn, world raster pixels; `(-1, -1)` when it is not.
    #[func]
    fn animal_at(&self, id: i64) -> Vector2 {
        u64::try_from(id)
            .ok()
            .and_then(|id| self.hosted.as_ref()?.animal_at(id))
            .map_or(Vector2::new(-1.0, -1.0), |(x, y)| Vector2::new(x, y))
    }

    /// What the presenter has spent: `frames`, and cumulative milliseconds `record_ms` (CPU),
    /// `gpu_ms`, `copy_ms` (the read-back copy), `pack_ms` over `packs` (on the pack thread),
    /// `snapshot_ms` over `snapshots` (the loop's copy for the pack); `packs_refused`,
    /// `displaced` (frames posted over an untaken one), `unlit` (a still world drawn before its
    /// sky plane arrived).
    #[func]
    fn stats(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(h) = &self.hosted {
            let s = h.stats();
            d.set("frames", &(s.frames as i64).to_variant());
            d.set("record_ms", &s.record_ms.to_variant());
            d.set("gpu_ms", &s.gpu_ms.to_variant());
            d.set("copy_ms", &s.copy_ms.to_variant());
            d.set("pack_ms", &s.pack_ms.to_variant());
            d.set("packs", &(s.packs as i64).to_variant());
            d.set("snapshot_ms", &s.snapshot_ms.to_variant());
            d.set("snapshots", &(s.snapshots as i64).to_variant());
            d.set("packs_refused", &(s.packs_refused as i64).to_variant());
            d.set("displaced", &(s.displaced as i64).to_variant());
            d.set("unlit", &s.unlit.to_variant());
        }
        d
    }

    /// The last thing that went wrong, or "".
    #[func]
    fn error(&self) -> GString {
        self.hosted.as_ref().and_then(|h| h.error()).map_or_else(GString::new, |e| GString::from(e.as_str()))
    }

    /// Free the GPU renderer; the frames already taken stay. A still thumbnail does this.
    #[func]
    fn release(&self) {
        if let Some(h) = &self.hosted {
            h.release();
        }
    }
}

impl WorldRenderer {
    fn show_static(&mut self, session: Gd<CubariumSession>, index: i64, queued: bool) {
        self.detach();
        let (Some(h), Ok(i)) = (&self.hosted, usize::try_from(index)) else { return };
        h.show_static(session.bind().session.candidate_source(), i, queued);
    }
}

impl Drop for WorldRenderer {
    fn drop(&mut self) {
        self.detach();
    }
}
