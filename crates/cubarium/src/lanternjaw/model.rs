use cubarium_render::Sprite;
use cubarium_surface::Vec2;

/// The four presentation modes of the study. Names describe choreography, not ecology: a
/// `Hunt` loop is one articulated strike in a six-second cycle and says nothing about prey,
/// a `Bud` shows a cocoon under the tail and says nothing about gestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Rest,
    Move,
    Hunt,
    Bud,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Rest, Mode::Move, Mode::Hunt, Mode::Bud];
}

/// The parts of the rig in **painting order** (back to front), with their depth layer:
///
/// | part | layer | content |
/// | --- | --- | --- |
/// | `FarLimb` | 0 | the far raptorial forelimb, 45 ms behind, one pixel higher, 55 % alpha |
/// | `Underside` | 1 | the three walking-leg pairs and, in `Bud`, the cocoon |
/// | `Tail` | 2 | hull template columns 0..=3 (`dx` −9..=−6): the fan and the first rim |
/// | `Abdomen` | 2 | columns 4..=8 (`dx` −5..=−1): three lanterns and their seams |
/// | `Thorax` | 2 | columns 9..=12 (`dx` 0..=3): two lanterns and their seams |
/// | `Head` | 2 | columns 13..=17 (`dx` 4..=8): brow lamp, crown, plates, eye, jaw |
/// | `Glow` | 3 | the lantern halos (one row up) and glows (two rows up) of every lantern |
/// | `NearLimb` | 4 | the near forelimb, its claw in the claw colour |
///
/// A hull template cell is rasterized into the piece its **column** belongs to whatever its
/// displacement (the pieces have padding), so a coil that compresses the hull is still one
/// summed material. Each hull piece's `offset` is the integer body-local point `(left column's
/// dx + 2, 0)` (for the five-column pieces; `(−7, 0)` for the tail) and its pivot is at
/// integer sprite coordinates, so all four pieces share the body lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartName {
    FarLimb,
    Underside,
    Tail,
    Abdomen,
    Thorax,
    Head,
    Glow,
    NearLimb,
}

impl PartName {
    /// Painting order.
    pub const ALL: [PartName; 8] = [
        PartName::FarLimb,
        PartName::Underside,
        PartName::Tail,
        PartName::Abdomen,
        PartName::Thorax,
        PartName::Head,
        PartName::Glow,
        PartName::NearLimb,
    ];

    /// The depth layer of [`cubarium_render::RigPart::layer`], per the table above.
    pub fn layer(self) -> u8 {
        match self {
            PartName::FarLimb => 0,
            PartName::Underside => 1,
            PartName::Tail | PartName::Abdomen | PartName::Thorax | PartName::Head => 2,
            PartName::Glow => 3,
            PartName::NearLimb => 4,
        }
    }
}

/// One rasterized part for one frame: the sprite and the body-local position of its pivot
/// (see [`cubarium_render::RigPart`] for the body frame).
#[derive(Clone, Debug)]
pub struct Part {
    pub name: PartName,
    pub sprite: Sprite,
    pub offset: Vec2,
}

impl Part {
    /// This part as the renderer sees it, at its name's layer.
    pub fn rig_part(&self) -> cubarium_render::RigPart<'_> {
        cubarium_render::RigPart {
            sprite: &self.sprite,
            offset: self.offset,
            layer: self.name.layer(),
        }
    }
}
