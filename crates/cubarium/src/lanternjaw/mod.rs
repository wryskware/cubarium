//! Lanternjaw — the selected megafauna body, drawn code-natively as a multipart rig.
//!
//! **Presentation only.** This module draws a picture: it is not a founder, a species, a
//! diet, a capability or a hunting outcome, and nothing in `cubarium-core` knows it exists.
//! It is not in the atelier pack and no rig index selects it, so no saved genome, no
//! founder and no `rig_of` fallback changes because it is here. It exists so that a later,
//! separately validated predator experiment has an actual reusable body to draw with —
//! the production form of the art study in `art/studies/megafauna/fable.js` (Wrysk's
//! preferred candidate, 2026-09-12), whose look this module preserves and whose
//! whole-pixel stepped motion it deliberately does **not** carry over.
//!
//! # What is preserved from the study
//!
//! The 18 × 5 hull template (`ROWS`), the nine-plate segmentation, the cyan lantern chain
//! with its travelling crest, the folded raptorial forelimbs that carry light only as they
//! extend, three walking-leg pairs, the tail fan, the bud cocoon, the four modes and every
//! rhythm, envelope and colour of `fable.md` "Footprint and timing" and "Palette". The warm
//! accent still appears only at the peak of the hunt's 240 ms strike envelope.
//!
//! # What changes for production
//!
//! * **Fractional motion.** The study rounded every column offset, every limb joint and the
//!   anchor to whole pixels. Here the sinuous wave `dy_i`, the coil/lunge `dx_i`, the limb
//!   joints and the leg swing are used **unrounded**: every template cell is splatted
//!   bilinearly at its fractional position, the limbs are anti-aliased polylines, and the
//!   anchor is whatever fractional [`SurfacePoint`] the caller passes, so motion reads as
//!   fractional brightness at 60 fps the way the world's bodies already do. The one binary
//!   quantity in the study, the leg lift (`u < 0.38 ? 1 : 0`), becomes the continuous bump
//!   `sin(π · u / 0.38)` for `u < 0.38`, 0 otherwise.
//! * **The study's colours, decoded once; alpha where the study meant translucency.** Every
//!   colour mix is computed in sRGB exactly as `fable.js` computes it (including its mixes
//!   toward the `#0B0525` background for the opaque structure — rim, plates, seams, head
//!   plates, eye, jaw, lantern sockets, legs, near limb, claw — which Astra asked to keep
//!   as deliberately dark opaque shell), then decoded with [`cubarium_render::srgb_decode`]
//!   into premultiplied linear. Only the tail fan, the lantern halo and glow, the cocoon
//!   and the far limb carry real alpha (the study's mix fraction over the pure colour), so
//!   those compose over the cube's real ground instead of painting a background-coloured
//!   block. Over bright water the dark carapace therefore reads as a dark silhouette; see
//!   `captures/lanternjaw/grounds.png`.
//! * **Parts through one query.** The body is eight bounded sprites (see [`PartName`]) each
//!   rasterized fresh for the frame in body coordinates and drawn by
//!   [`cubarium_render::stamp_rig`] through **one** root-owned pixel query: the hull's four
//!   pieces are one material (one layer, summed — they are cut from one lattice, so the sum
//!   is the uncut hull), and the far limb, the underside, the glow and the near limb are
//!   separate depths composited in that order. No part is stamped from its own anchor.
//!
//! # Footprint
//!
//! **Normative.** In every mode at every time, every painted texel centre of every part, in
//! body-local pixels from the anchor (body `+x` forward, `+y` down when facing image-right),
//! lies within `x ∈ [−BOUND_BACK, BOUND_FRONT]`, `y ∈ [−BOUND_ABOVE, BOUND_BELOW]`; every
//! part's sprite, from its own pivot, has an extent of at most [`PART_EXTENT_MAX`]; and the
//! rig's query radius ([`cubarium_render::rig_radius`]) is at most [`QUERY_RADIUS_MAX`]. A
//! part that could not be built within its budget is a bug, not a skipped frame:
//! [`Lanternjaw::parts`] always returns exactly the eight parts of [`PartName::ALL`].
//!
//! The hull pieces, the underside and the glow share the **body lattice**: their `offset`s
//! and their sprites' pivots have integer coordinates, so every texel centre of those parts
//! sits at a half-integer body coordinate exactly as a template cell does. This is what makes
//! the hull's four pieces sum to the uncut hull under bilinear sampling.

mod envelopes;
mod living;
mod model;
mod palette;
mod raster;

#[cfg(test)]
mod tests;

pub use envelopes::*;
pub use living::*;
pub use model::*;
pub use raster::*;
