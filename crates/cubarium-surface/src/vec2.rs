//! Chart-tangent vectors and the orthogonal maps that transport them across seams.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// A vector in a face chart's tangent plane, in pixel units: `x` along `+u` (image-right),
/// `y` along `+v` (image-down).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[inline]
    pub const fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    #[inline]
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }

    #[inline]
    pub fn length_sq(self) -> f64 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f64 {
        self.length_sq().sqrt()
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// `Vec2::new(cos a, sin a)` for a heading angle measured counter-clockwise on screen
    /// from `+u`, i.e. angle `a` points image-right at 0 and image-up at `π/2`.
    /// Because `+v` is image-down this is `(cos a, -sin a)`.
    pub fn from_screen_angle(a: f64) -> Vec2 {
        Vec2::new(a.cos(), -a.sin())
    }

    /// The counter-clockwise-on-screen angle of this vector (inverse of
    /// [`Vec2::from_screen_angle`]), in `(-π, π]`.
    pub fn screen_angle(self) -> f64 {
        (-self.y).atan2(self.x)
    }

    /// Rotate `turns` quarter turns counter-clockwise on screen, exactly like
    /// `cube_proto::rotate_heading`: one turn maps `(x, y)` to `(y, -x)`.
    #[inline]
    pub fn rotate_quarter_turns(self, turns: u8) -> Vec2 {
        match turns % 4 {
            0 => self,
            1 => Vec2::new(self.y, -self.x),
            2 => Vec2::new(-self.x, -self.y),
            _ => Vec2::new(-self.y, self.x),
        }
    }

    /// Normalized copy, or `None` when the length is not positive and finite.
    pub fn normalized(self) -> Option<Vec2> {
        let l = self.length();
        (l > 0.0 && l.is_finite()).then(|| self * (1.0 / l))
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}
impl Mul<f64> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    #[inline]
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// An orthogonal 2×2 map with entries in `{-1, 0, 1}`: the net effect on tangent
/// vectors of a sequence of seam rotations and rim reflections.
///
/// Row-major: `x' = m[0][0]·x + m[0][1]·y`, `y' = m[1][0]·x + m[1][1]·y`. The group is
/// the dihedral group of the square, so any composition stays representable exactly.
/// Callers apply the same map to every tangent quantity that traveled (heading,
/// steering vectors, local directional memory); scalars and body-relative values
/// are untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TangentMap {
    pub m: [[i8; 2]; 2],
}

impl TangentMap {
    pub const IDENTITY: TangentMap = TangentMap { m: [[1, 0], [0, 1]] };

    /// Counter-clockwise-on-screen rotation by `turns` quarter turns (`rotate_heading`).
    pub const fn quarter_turns(turns: u8) -> TangentMap {
        match turns % 4 {
            0 => TangentMap::IDENTITY,
            1 => TangentMap { m: [[0, 1], [-1, 0]] },
            2 => TangentMap { m: [[-1, 0], [0, -1]] },
            _ => TangentMap { m: [[0, -1], [1, 0]] },
        }
    }

    /// Reflection across a horizontal line: `(x, y) -> (x, -y)`. This is the rim
    /// reflection at a side face's bottom edge.
    pub const REFLECT_Y: TangentMap = TangentMap { m: [[1, 0], [0, -1]] };

    /// Reflection across a vertical line: `(x, y) -> (-x, y)`.
    pub const REFLECT_X: TangentMap = TangentMap { m: [[-1, 0], [0, 1]] };

    #[inline]
    pub fn apply(self, v: Vec2) -> Vec2 {
        let m = self.m;
        Vec2::new(
            f64::from(m[0][0]) * v.x + f64::from(m[0][1]) * v.y,
            f64::from(m[1][0]) * v.x + f64::from(m[1][1]) * v.y,
        )
    }

    /// The map that applies `self` first and then `next`: `(next ∘ self)`.
    pub const fn then(self, next: TangentMap) -> TangentMap {
        let a = self.m;
        let b = next.m;
        TangentMap {
            m: [
                [b[0][0] * a[0][0] + b[0][1] * a[1][0], b[0][0] * a[0][1] + b[0][1] * a[1][1]],
                [b[1][0] * a[0][0] + b[1][1] * a[1][0], b[1][0] * a[0][1] + b[1][1] * a[1][1]],
            ],
        }
    }

    /// The inverse map; for an orthogonal integer matrix this is the transpose.
    pub const fn inverse(self) -> TangentMap {
        let m = self.m;
        TangentMap { m: [[m[0][0], m[1][0]], [m[0][1], m[1][1]]] }
    }

    /// Determinant: `+1` for a pure rotation, `-1` when an odd number of reflections
    /// is included.
    pub const fn det(self) -> i8 {
        self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]
    }

    /// `Some(turns)` when this map is a pure quarter-turn rotation.
    pub fn as_quarter_turns(self) -> Option<u8> {
        (0..4u8).find(|&k| TangentMap::quarter_turns(k) == self)
    }
}

impl Default for TangentMap {
    fn default() -> Self {
        TangentMap::IDENTITY
    }
}
