//! The weather and the time of day as the voxel renderer draws them (package WX2,
//! `design/handoffs/voxel-weather-2026-09-24.md`). **Interim look**: every colour below is
//! the palette's (`crates/cubarium/src/voxel/present.rs` and the species colours), and
//! every gain and time constant is a placeholder for Wrysk to judge from GIFs.
//!
//! Three steps, all on the CPU and all cheap:
//!
//! 1. [`Weather`]: what the simulation says (the renderer's copy of
//!    `cubarium_voxel::weather::WeatherView`; this crate depends on no world crate).
//! 2. [`WeatherEase`]: that, eased on the every-frame clock, so **nothing pops**: a new
//!    world, a front or a rain mode moves the picture over seconds, never between frames.
//! 3. [`look`]: the eased weather turned into what the shader reads: the sky's gradient
//!    and its dawn/dusk band, the ambient and sun light, the clouds' colours, the flat
//!    tier's tint. At a clear noon it is exactly today's picture: the palette's sky, haze
//!    and ambient, the configured sun, and a flat tint of one.
//!
//! The sun arcs through the front hemisphere (it must: a sun behind the scene leaves
//! every drawn front face in self-shadow). At noon it is the configured `[light] sun`; it
//! rises far to the left ([`AZ_RISE`]) and sets to the front right ([`AZ_SET`]), so the
//! long shadows of dawn fall right and those of dusk fall back and left. At night a weak
//! cool moon takes the sun's slot, half a day behind it.

use crate::voxel::VoxelParams;

/// What is falling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rain {
    #[default]
    Clear,
    Drizzle,
    Shower,
    Downpour,
}

/// The most recent lightning strike (`cubarium_voxel::weather::Strike`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strike {
    pub tick: u64,
    pub x: u32,
    pub z: u32,
    pub seed: u32,
}

/// The weather to draw: `cubarium_voxel::weather::WeatherView`'s drawn fields, with the
/// same meanings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    pub rain: Rain,
    pub rain_m_per_s: f32,
    pub fog: f32,
    pub cloud_cover: f32,
    /// 0 midnight, 0.5 noon, wrapping.
    pub day_phase: f32,
    pub daylight: f32,
    /// Radians; negative below the horizon.
    pub sun_elevation: f32,
    pub last_strike: Option<Strike>,
}

impl Weather {
    /// A clear noon: today's picture.
    pub const CLEAR_NOON: Weather = Weather {
        rain: Rain::Clear,
        rain_m_per_s: 0.0,
        fog: 0.0,
        cloud_cover: 0.0,
        day_phase: 0.5,
        daylight: 1.0,
        sun_elevation: core::f32::consts::FRAC_PI_2,
        last_strike: None,
    };
}

impl Default for Weather {
    fn default() -> Weather {
        Weather::CLEAR_NOON
    }
}

// --- easing ------------------------------------------------------------------------------

/// Seconds of sim time for the day's own quantities (phase, daylight, the sun) to close
/// most of a jump: they move continuously in the simulation, so this only smooths a
/// discontinuity (a new world, a clock mode change).
pub const SKY_TAU_S: f32 = 0.5;
/// Seconds for cloud cover to follow the view: clouds build and clear over a minute.
pub const CLOUD_TAU_S: f32 = 12.0;
/// Seconds for the storm's darkening to follow a downpour's start and end.
pub const STORM_TAU_S: f32 = 8.0;
/// How far clouds drift, voxels a second of sim time (along +x, round the ring).
pub const CLOUD_DRIFT: f64 = 0.35;
/// The shader's twinkle clock wraps at this many seconds; every twinkle has a whole
/// number of periods in it, so the wrap is invisible.
pub const TWINKLE_WRAP_S: f64 = 3600.0;

/// The weather as drawn this frame: every quantity eased toward the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eased {
    pub day_phase: f32,
    pub daylight: f32,
    pub sun_elevation: f32,
    pub cloud_cover: f32,
    /// 1 through a downpour, 0 outside one.
    pub storm: f32,
    /// How far the clouds have drifted, voxels, wrapped at the ring's width.
    pub drift: f32,
    /// Sim seconds, wrapped at [`TWINKLE_WRAP_S`].
    pub seconds: f32,
}

/// Follows a [`Weather`] on the frame clock.
#[derive(Clone, Debug, Default)]
pub struct WeatherEase {
    state: Option<Eased>,
    drift: f64,
    seconds: f64,
}

fn ease(from: f32, to: f32, dt: f32, tau: f32) -> f32 {
    if dt <= 0.0 {
        return from;
    }
    from + (to - from) * (1.0 - (-dt / tau).exp())
}

impl WeatherEase {
    /// Move `dt` seconds of sim time toward `target` and return the eased weather. The
    /// first call takes the target as it is (nothing to ease from). `width` is the ring's
    /// width in voxels, where the cloud drift wraps.
    pub fn advance(&mut self, target: &Weather, dt: f32, width: u32) -> Eased {
        let dt = if dt.is_finite() { dt.clamp(0.0, 1.0) } else { 0.0 };
        let storm = if target.rain == Rain::Downpour { 1.0 } else { 0.0 };
        self.seconds = (self.seconds + f64::from(dt)).rem_euclid(TWINKLE_WRAP_S);
        self.drift =
            (self.drift + f64::from(dt) * CLOUD_DRIFT).rem_euclid(f64::from(width.max(1)));
        let next = match self.state {
            None => Eased {
                day_phase: target.day_phase.rem_euclid(1.0),
                daylight: target.daylight,
                sun_elevation: target.sun_elevation,
                cloud_cover: target.cloud_cover,
                storm,
                drift: 0.0,
                seconds: 0.0,
            },
            Some(s) => {
                // The phase wraps: ease along the shorter way round.
                let d = (target.day_phase - s.day_phase + 0.5).rem_euclid(1.0) - 0.5;
                Eased {
                    day_phase: (s.day_phase + ease(0.0, d, dt, SKY_TAU_S)).rem_euclid(1.0),
                    daylight: ease(s.daylight, target.daylight, dt, SKY_TAU_S),
                    sun_elevation: ease(s.sun_elevation, target.sun_elevation, dt, SKY_TAU_S),
                    cloud_cover: ease(s.cloud_cover, target.cloud_cover, dt, CLOUD_TAU_S),
                    storm: ease(s.storm, storm, dt, STORM_TAU_S),
                    drift: 0.0,
                    seconds: 0.0,
                }
            }
        };
        let next = Eased {
            drift: self.drift as f32,
            seconds: self.seconds as f32,
            ..next
        };
        self.state = Some(next);
        next
    }

    /// The eased weather as of the last [`WeatherEase::advance`], if there was one.
    pub fn current(&self) -> Option<Eased> {
        self.state
    }
}

// --- the look ------------------------------------------------------------------------------

/// The sun's elevation at noon in the day clock (`[world.day]`: the sine peaks at 70°).
/// The drawn sun reaches the configured `[light] sun`'s own elevation there.
pub const NOON_ELEVATION_DEG: f32 = 70.0;
/// Where the sun rises and sets, degrees of azimuth from straight toward the camera,
/// positive to the right. Both in the front hemisphere.
pub const AZ_RISE_DEG: f32 = -80.0;
pub const AZ_SET_DEG: f32 = 60.0;
/// The share of the day (either side of noon) over which the sun swings from its noon
/// azimuth to its rising or setting one.
pub const AZ_SWING: f32 = 0.3;
/// The lowest the drawn sun goes for shadows, degrees: below it the shadows would run the
/// length of the world (and the march with them); the sun fades out there instead.
pub const MIN_SHADOW_DEG: f32 = 12.0;
/// The sun's strength fades in over this elevation above the horizon, degrees.
pub const SUN_FADE_DEG: f32 = 6.0;
/// The moon's strength as a share of the sun's.
pub const MOON: f32 = 0.3;
/// The ambient light at night as a share of the day's.
pub const NIGHT_AMBIENT: f32 = 0.34;
/// The night sky's zenith and horizon as shares of the day's.
pub const NIGHT_SKY: f32 = 0.5;
pub const NIGHT_HORIZON: f32 = 0.45;
/// How far haze darkens at night (a share of the day's haze colour).
pub const NIGHT_HAZE: f32 = 0.35;
/// What the unlit water (falls, the lake's cut face, foam) keeps at night.
pub const NIGHT_UNLIT: f32 = 0.45;
/// The twilight band's brightness at its peak (sun on the horizon), linear light.
pub const BAND_GAIN: f32 = 0.6;
/// How wide the twilight is, degrees of sun elevation either side of the horizon.
pub const TWILIGHT_DEG: f32 = 12.0;
/// How much a full overcast dims the sun and the ambient.
pub const OVERCAST_SUN: f32 = 0.85;
pub const OVERCAST_AMBIENT: f32 = 0.15;
/// How far a sunlit texel leans toward the warm light when the sun is on the horizon.
pub const WARM_LEAN: f32 = 0.22;
/// How far a moonlit texel leans toward the moon's cool light.
pub const MOON_LEAN: f32 = 0.05;

/// The day sky's weight comes in as the sun climbs from here (degrees; the twilight band
/// has faded most of the way) ...
pub const DAY_SKY_FROM_DEG: f32 = 6.0;
/// ... and is whole from here.
pub const DAY_SKY_TO_DEG: f32 = 28.0;
/// How far a full overcast greys and dims the day sky, and how far a storm darkens it.
pub const OVERCAST_SKY_GREY: f32 = 0.4;
pub const OVERCAST_SKY_DIM: f32 = 0.25;
pub const STORM_SKY_DIM: f32 = 0.55;
/// `[light] star_bloom` by default: how much of the brightest stars' light feeds the
/// bloom (0 skips it).
pub const STAR_BLOOM_DEFAULT: f32 = 0.15;

/// The lit tier's daytime sky (WX2 checkpoint 1b, `[light] day_sky`): interim variants
/// for Wrysk to choose between. Every one reads as day by **value** (pale, high-key, the
/// horizon lightest) and parts from the violet-blue terrain by **hue**; none is a blue
/// sky (Wrysk, 2026-09-24: blue behind blue-violet terrain loses the ridge).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DaySky {
    /// Wrysk's pick (2026-09-25): a red dwarf's day. A coral-apricot horizon through a
    /// salmon-rose middle to a pale lilac zenith; dimmer and redder than the first
    /// apricot haze, the terrain's complement, kin to the sunsets.
    #[default]
    Warm,
    /// A pale electric mint horizon to an aqua-teal zenith.
    Mint,
    /// A near-white lavender haze: the ridge a darker silhouette against it.
    Lilac,
}

impl DaySky {
    /// Zenith, middle and horizon (sRGB).
    pub fn srgb(self) -> [u32; 3] {
        match self {
            DaySky::Warm => [0xCC_B0_D8, 0xE0_A8_A8, 0xF2_BC_96],
            DaySky::Mint => [0x55_C6_CE, 0x98_EC_E0, 0xD8_FF_F2],
            DaySky::Lilac => [0xC2_B6_EA, 0xDD_D4_F6, 0xF6_F1_FF],
        }
    }

    /// The clouds by day: their near-white body and their shadowed underside (sRGB).
    pub fn cloud_srgb(self) -> [u32; 2] {
        match self {
            DaySky::Warm => [0xF7_E0_D6, 0x7E_64_B4],
            DaySky::Mint => [0xF6_FF_FC, 0x84_78_C8],
            DaySky::Lilac => [0xFF_FF_FF, 0x86_72_C6],
        }
    }
}

/// Palette colours the weather adds (sRGB), all already in the game's palette.
pub const MAGENTA_SRGB: u32 = 0xFF2AFC;
pub const ORANGE_SRGB: u32 = 0xFF9B50;
pub const PURPLE_SRGB: u32 = 0x59206B;
pub const DEEP_BLUE_SRGB: u32 = 0x1E2798;
pub const BEDROCK_SRGB: u32 = 0x1A1038;

/// A low sun's light: the palette's orange a quarter of the way to its magenta.
fn warm_light() -> [f32; 3] {
    lerp3(srgb(ORANGE_SRGB), srgb(MAGENTA_SRGB), 0.25)
}

fn srgb(hex: u32) -> [f32; 3] {
    let ch = |s: u32| {
        let c = ((hex >> s) & 0xFF) as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [ch(16), ch(8), ch(0)]
}

/// `a·(1−t) + b·t`: exactly `a` at 0 and exactly `b` at 1.
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] * (1.0 - t) + b[i] * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn scale3(a: [f32; 3], k: f32) -> [f32; 3] {
    a.map(|c| c * k)
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// `c` at unit luminance.
fn unit(c: [f32; 3]) -> [f32; 3] {
    let l = luma(c);
    if l > 0.0 { c.map(|k| k / l) } else { [1.0; 3] }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The unit direction toward a body at azimuth `az` (from straight toward the camera,
/// positive right) and elevation `el`, radians: x right, y up, z into the scene.
fn direction(az: f32, el: f32) -> [f32; 3] {
    [az.sin() * el.cos(), el.sin(), -az.cos() * el.cos()]
}

/// Everything the shader reads about the weather and the time of day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The sky's zenith and horizon colours (linear): the palette's by day.
    pub sky: [f32; 3],
    pub sky_horizon: [f32; 3],
    /// The lit tier's haze colour.
    pub haze: [f32; 3],
    /// The ambient light's hue at unit luminance, and its level (1 by day).
    pub ambient: [f32; 3],
    pub ambient_level: f32,
    /// The unit direction toward the sun (or the moon), zero for none.
    pub sun: [f32; 3],
    /// The sun's share of the light, 0 to 1 (1 at a clear noon).
    pub sun_strength: f32,
    /// What a sunlit texel leans toward (`[light] sun_tint`): the palette's light by day,
    /// warm near the horizon, the moon's at night. The light itself stays the ambient's
    /// hue: its colour is this lean (a colour of its own in the walk costs occupancy).
    pub sun_lean: [f32; 3],
    /// How far a sunlit texel leans, times its N·L: the sun tint times the sun's strength,
    /// plus the low sun's warm lean over the sun's own height (so a top, whose N·L is that
    /// height, leans by the warm lean itself).
    pub sun_tint: f32,
    /// -1 when the sun is to the left, 1 to the right: which side the band leans to.
    pub sun_side: f32,
    /// How bright the stars are (0 by day).
    pub stars: f32,
    /// The twilight band: its strength, and its three colours from the top down.
    pub band: f32,
    pub band_colours: [[f32; 3]; 3],
    /// The clouds: cover, how stormy, drift (voxels), twinkle clock (seconds).
    pub cloud_cover: f32,
    pub storm: f32,
    pub drift: f32,
    pub seconds: f32,
    /// The clouds' body colour, their sunlit edge colour and how strong the edge is.
    pub cloud_body: [f32; 3],
    pub cloud_lit: [f32; 3],
    pub cloud_edge: f32,
    /// The flat tier's whole-picture tint (1 by day).
    pub tint: [f32; 3],
    /// What the lit tier's unlit water keeps (1 by day).
    pub unlit: f32,
    /// The lit tier's day sky ([`DaySky`]): how much of it shows (1 once the sun is well
    /// up, 0 at twilight and night), and its zenith, middle and horizon (linear, already
    /// greyed and dimmed by the cloud cover and the storm).
    pub day: f32,
    pub day_sky: [[f32; 3]; 3],
    /// The clouds by day: their body and their shadowed underside (linear).
    pub cloud_day: [f32; 3],
    pub cloud_shade: [f32; 3],
    /// How much of the brightest stars' light feeds the bloom (`[light] star_bloom`).
    pub star_bloom: f32,
}

impl Look {
    /// Whether anything in the sky moves on its own from frame to frame (drifting clouds,
    /// twinkling stars): the lit tier then redraws every frame.
    pub fn animated(&self) -> bool {
        self.cloud_cover > 1e-3 || self.stars > 1e-3
    }

    /// Whether `self` and `other` draw the same picture: equal, but for the drift and the
    /// twinkle clock when nothing in either sky moves on them.
    pub fn same_picture(&self, other: &Look) -> bool {
        if self.animated() || other.animated() {
            return self == other;
        }
        Look { drift: 0.0, seconds: 0.0, ..*self } == Look { drift: 0.0, seconds: 0.0, ..*other }
    }
}

/// The look of a clear noon: today's picture.
pub fn noon(p: &VoxelParams) -> Look {
    look(&WeatherEase::default().advance(&Weather::CLEAR_NOON, 0.0, p.width), p)
}

/// The picture's weather and time of day for `e`, over the palette and the `[light]`
/// knobs in `p`.
pub fn look(e: &Eased, p: &VoxelParams) -> Look {
    let dl = e.daylight.clamp(0.0, 1.0);
    let c = e.cloud_cover.clamp(0.0, 1.0);
    let el_deg = e.sun_elevation.to_degrees();

    // Twilight: a bell over the sun's crossing of the horizon, zero far from it (so a clear
    // noon is exactly the day's look).
    let tw = if el_deg.abs() < 4.0 * TWILIGHT_DEG {
        (-(el_deg / TWILIGHT_DEG).powi(2)).exp()
    } else {
        0.0
    };

    // --- the sun (or the moon) ---
    let len = (p.sun[0] * p.sun[0] + p.sun[1] * p.sun[1] + p.sun[2] * p.sun[2]).sqrt();
    let have_sun = len > 0.0 && p.sun[1] > 0.0;
    let noon_el = if have_sun { (p.sun[1] / len).asin() } else { 0.0 };
    let az0 = if have_sun { p.sun[0].atan2(-p.sun[2]) } else { 0.0 };
    let place = |phase: f32, el_view: f32| -> [f32; 3] {
        let s = phase.rem_euclid(1.0) - 0.5;
        let u = (s.abs() / AZ_SWING).min(1.0);
        let end = if s < 0.0 { AZ_RISE_DEG } else { AZ_SET_DEG }.to_radians();
        let az = lerp(az0, end, u);
        let el = (el_view * noon_el / NOON_ELEVATION_DEG.to_radians())
            .min(noon_el)
            .max(MIN_SHADOW_DEG.to_radians().min(noon_el));
        if s == 0.0 && el_view * noon_el / NOON_ELEVATION_DEG.to_radians() >= noon_el {
            // Noon: the configured sun itself.
            return p.sun;
        }
        direction(az, el)
    };
    let fade = |deg: f32| smoothstep(0.0, SUN_FADE_DEG, deg);
    let clouded = 1.0 - OVERCAST_SUN * c.powf(1.5);

    let day_hue = p.ambient_colour;
    let indigo = lerp3(srgb(DEEP_BLUE_SRGB), srgb(PURPLE_SRGB), 0.35);
    let night_hue = unit(lerp3([1.0; 3], indigo, 0.75));
    let twilight_hue = unit(lerp3([1.0; 3], srgb(MAGENTA_SRGB), 0.45));
    let mut ambient = lerp3(night_hue, day_hue, dl);
    if tw > 0.0 {
        ambient = unit(lerp3(ambient, twilight_hue, 0.2 * tw));
    }

    // The extra lean: a low sun's warm light, the moon's cool one.
    let (sun, sun_strength, sun_lean, extra_lean) = if !have_sun {
        ([0.0; 3], 0.0, p.light, 0.0)
    } else if e.sun_elevation >= 0.0 {
        // Warm near the horizon, the palette's light high up.
        let warm = smoothstep(30.0, 2.0, el_deg);
        let lean = if warm > 0.0 {
            // Straight to the warm colour: halfway between cyan and orange is a grey.
            lerp3(p.light, warm_light(), (1.6 * warm).min(1.0))
        } else {
            p.light
        };
        let strength = fade(el_deg) * clouded;
        (place(e.day_phase, e.sun_elevation), strength, lean, WARM_LEAN * warm * strength)
    } else {
        let moon_el = -e.sun_elevation;
        let fade_moon = fade(moon_el.to_degrees()) * clouded;
        (
            place(e.day_phase + 0.5, moon_el),
            MOON * fade_moon,
            lerp3(p.light, [1.0; 3], 0.4),
            MOON_LEAN * fade_moon,
        )
    };
    // No strength, no sun: the walk skips the shadow march for a zero direction.
    let sun = if sun_strength > 0.0 { sun } else { [0.0; 3] };
    let sun_tint = if extra_lean > 0.0 {
        // Over the larger N·L of a top (the sun's height) and a front (its nearness to the
        // camera), so no face leans by more than the extra lean itself.
        (p.sun_tint * sun_strength + extra_lean / sun[1].max(-sun[2]).max(0.25)).min(1.0)
    } else {
        p.sun_tint * sun_strength
    };

    let ambient_level = lerp(NIGHT_AMBIENT, 1.0, dl) * (1.0 - OVERCAST_AMBIENT * c);

    // --- the sky ---
    let sky = lerp3(scale3(p.sky, NIGHT_SKY), p.sky, dl);
    let night_horizon = lerp3(scale3(p.sky_horizon, NIGHT_HORIZON), scale3(srgb(DEEP_BLUE_SRGB), 0.02), 0.3);
    let sky_horizon = lerp3(night_horizon, p.sky_horizon, dl);
    let mut haze = lerp3(scale3(p.haze_colour, NIGHT_HAZE), p.haze_colour, dl);
    if tw > 0.0 {
        haze = lerp3(haze, srgb(PURPLE_SRGB), 0.3 * tw);
    }
    let band = BAND_GAIN * tw * (1.0 - 0.75 * c);
    let band_colours = [srgb(PURPLE_SRGB), srgb(MAGENTA_SRGB), srgb(ORANGE_SRGB)];
    let stars = (1.0 - dl) * (1.0 - dl);

    // --- the clouds ---
    // By day a lifted haze, lit at the edges by the palette's light; at night a little
    // over the night horizon, edged by the moon; at twilight edged warm.
    let body_day = scale3(p.haze_colour, 2.2);
    let body_night = scale3(p.sky_horizon, 1.6);
    let cloud_body = lerp3(body_night, body_day, dl);
    let lit_day = scale3(p.light, 0.22);
    let lit_night = scale3(lerp3(p.light, [1.0; 3], 0.3), 0.025);
    let mut cloud_lit = lerp3(lit_night, lit_day, dl);
    if tw > 0.0 {
        let warm = lerp3(srgb(MAGENTA_SRGB), srgb(ORANGE_SRGB), 0.5);
        cloud_lit = lerp3(cloud_lit, scale3(warm, 0.3), tw);
    }
    let cloud_edge = lerp(0.6, 1.0, tw);

    // --- the flat tier's tint and the lit tier's unlit water ---
    let tint = [0, 1, 2].map(|i| ambient_level * ambient[i] / day_hue[i].max(1e-6));
    let tint = if dl >= 1.0 && tw == 0.0 && c == 0.0 { [1.0; 3] } else { tint };
    let unlit = lerp(NIGHT_UNLIT, 1.0, dl);

    // --- the lit tier's day sky ---
    // It comes in only once the twilight band has mostly gone, so dawn and dusk are
    // exactly checkpoint 1's; clouds grey and dim it, a storm darkens it.
    let day = if e.sun_elevation > 0.0 {
        smoothstep(DAY_SKY_FROM_DEG, DAY_SKY_TO_DEG, el_deg) * dl
    } else {
        0.0
    };
    let storm = e.storm.clamp(0.0, 1.0);
    let dim = (1.0 - OVERCAST_SKY_DIM * c) * (1.0 - STORM_SKY_DIM * storm);
    let day_sky = p.day_sky.srgb().map(|h| {
        let k = srgb(h);
        let grey = [luma(k); 3];
        scale3(lerp3(k, grey, OVERCAST_SKY_GREY * c), dim)
    });
    let [cloud_day, cloud_shade] = p.day_sky.cloud_srgb().map(srgb);

    Look {
        sky,
        sky_horizon,
        haze,
        ambient,
        ambient_level,
        sun,
        sun_strength,
        sun_lean,
        sun_tint,
        sun_side: sun[0].clamp(-1.0, 1.0),
        stars,
        band,
        band_colours,
        cloud_cover: c,
        storm: e.storm.clamp(0.0, 1.0),
        drift: e.drift,
        seconds: e.seconds,
        cloud_body,
        cloud_lit,
        cloud_edge,
        tint,
        unlit,
        day,
        day_sky,
        cloud_day,
        cloud_shade,
        star_bloom: p.star_bloom.max(0.0),
    }
}

#[cfg(test)]
mod tests;
