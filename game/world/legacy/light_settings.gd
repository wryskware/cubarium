class_name LightSettings
extends Resource
## **How a `WorldView` is lit**: one switch per Godot built-in effect, and every number the
## light uses as a named field. WorldView applies it (`WorldView.light`, set before the
## first view; see `set_light`).
##
## Every effect is off by default (the standing rule: a new lit effect waits for Wrysk's
## pick, and off skips its cost). The numbers below the switches apply whether or not an
## effect is on where they say so; the rest only matter while their effect is on.
##
## The old renderer's lit tier (`crates/cubarium-gpu`, `crates/cubarium/src/sink/gpu/`)
## marches the voxel grid for its shadows, reflections, light shafts, glow and AO. None of
## that is ported: these are Godot's own effects on the meshes, so they follow the
## animals' models as they turn. Where a number matches the old renderer, its comment
## names the old knob or constant.

# --- the switches -------------------------------------------------------------------------

## The sun (and the moon) casts shadows: a DirectionalLight3D shadow map over the world.
## The terrain is meshed closed (all six faces) so it casts correctly; that applies at the
## next world drawn.
@export var shadows := false
## Screen-space ambient occlusion (Environment.ssao).
@export var ssao := false
## Screen-space indirect light (Environment.ssil).
@export var ssil := false
## The sky drawn behind the world (a gradient by the time of day, as the old lit tier's
## warm day sky and dark night), instead of the plain dark background.
@export var sky := false
## The emitters glow (glowcap lips, bloomcrown cores, the lanternjaw's lure: their cells
## emit) and the view blooms (Environment.glow).
@export var glow := false
## Depth fog toward the back of the world, standing in for the old back-wall haze; while
## on, the mesher's baked vertex haze is off (it applies at the next world drawn).
@export var haze := false
## The water reflects the sky (a smooth, more specular water surface lit by the sky's
## radiance). Godot's SSR skips translucent surfaces, so this is the built-in that works.
@export var reflections := false
## Volumetric fog lit by the sun (Godot's froxel fog in a FogVolume over the world, so
## the empty space between the orthographic camera and the world stays clear), for light
## shafts.
@export var volumetric := false
## A shadowed point light (OmniLight3D) on each emitter (glowcaps, bloomcrown cores, the
## lanternjaw's lure), or one per group of them (`emitter_group_voxels`).
@export var emitter_lights := false
## Global illumination for caves and overhangs: SDFGI, or VoxelGI (`gi_mode`).
@export var gi := false

const SWITCHES := ["shadows", "ssao", "ssil", "sky", "glow", "haze", "reflections", "volumetric",
	"emitter_lights", "gi"]
## What `all` turns on: every switch but the two costly experiments (named on their own).
const ALL := ["shadows", "ssao", "ssil", "sky", "glow", "haze", "reflections", "volumetric"]

# --- the sun, the moon and the exposure (always applied) ---------------------------------

## The direction toward the sun at noon, in the old renderer's axes (x right, y up, z into
## the scene): `[light] sun`.
@export var noon_sun := Vector3(-1.0, 2.0, -1.0)
## The day clock's peak elevation, degrees (the old `NOON_ELEVATION_DEG`): the feed's
## elevation is scaled so this maps to the noon sun's own.
@export var noon_elevation_deg := 70.0
## Where the sun rises and sets, degrees of azimuth from straight toward the camera,
## positive right (old `AZ_RISE_DEG`, `AZ_SET_DEG`), and the share of the day either side
## of noon over which it swings there (old `AZ_SWING`).
@export var rise_azimuth_deg := -80.0
@export var set_azimuth_deg := 60.0
@export var azimuth_swing := 0.3
## The lowest the drawn sun goes, degrees (old `MIN_SHADOW_DEG`): lower, shadows would run
## the length of the world; the sun fades out instead.
@export var min_elevation_deg := 12.0
## The sun fades in over this elevation above the horizon, degrees (old `SUN_FADE_DEG`).
@export var sun_fade_deg := 6.0
## The sun's energy (DirectionalLight3D.light_energy) at full strength.
@export var sun_energy := 1.0
## The moon: the sun's light this share, opposite the sun in the day (old `MOON`).
@export var moon_share := 0.3
## How much a full overcast dims the sun and the ambient light (old `OVERCAST_SUN`,
## `OVERCAST_AMBIENT`).
@export var overcast_sun := 0.85
@export var overcast_ambient := 0.15
## A low sun's colour: toward this, by up to `sun_warm_share` as it sinks below 30 degrees
## (old `warm_light()` and `WARM_LEAN`'s ramp).
@export var sun_warm_colour := Color("ff8579")
@export var sun_warm_share := 0.6
## The ambient light's energy by day, and the share of it left at night (on top of the
## night colour's own dimness).
@export var ambient_energy := 1.0
@export var ambient_night_share := 1.0
## The ambient light's colour by day and at night. The night colour is the old night hue
## (white 0.75 of the way to its indigo) at the old `NIGHT_AMBIENT` 0.34 of the day's
## luminance, so it carries the night's dimming itself.
@export var ambient_day_colour := Color(1.0, 1.0, 1.0)
@export var ambient_night_colour := Color("9e99c0")
## Tonemapper (Environment.TONE_MAPPER_*: 0 linear, 1 Reinhard, 2 filmic, 3 ACES, 4 AgX)
## and its exposure.
@export var tonemap := 0
@export var exposure := 1.0

# --- shadows -------------------------------------------------------------------------------

## Shadow-map bias and normal bias (voxels are the unit: one voxel is 1.0).
@export var shadow_bias := 0.03
@export var shadow_normal_bias := 0.6
## How far the shadows soften (DirectionalLight3D.shadow_blur).
@export var shadow_blur := 1.0
## How dark a full shadow is, 0 none to 1 the sun wholly gone (shadow_opacity).
@export var shadow_opacity := 1.0
## The directional shadow map's size, texels square (one orthogonal split covers the
## world).
@export var shadow_map_size := 8192

# --- SSAO and SSIL -------------------------------------------------------------------------

@export var ssao_radius := 3.0
@export var ssao_intensity := 2.0
@export var ssao_power := 1.5
@export var ssil_radius := 6.0
@export var ssil_intensity := 1.0

# --- the sky ------------------------------------------------------------------------------

## The day sky top to bottom (the old lit tier's `day_sky = "warm"` stops) and the night
## sky (the palette's sky zenith and horizon, at the old `NIGHT_SKY` brightness).
@export var sky_day_top := Color("ccb0d8")
@export var sky_day_middle := Color("e0a8a8")
@export var sky_day_bottom := Color("f2bc96")
@export var sky_night_top := Color("0b0624")
@export var sky_night_bottom := Color("24153d")
## The day sky comes in as the sun climbs from the first elevation to the second, degrees
## (old `DAY_SKY_FROM_DEG`, `DAY_SKY_TO_DEG`).
@export var sky_day_from_deg := 6.0
@export var sky_day_to_deg := 28.0

# --- glow ---------------------------------------------------------------------------------

## An emitting cell's emission, times its own colour.
@export var emission_energy := 2.0
## Environment.glow: intensity, strength, bloom (share of the whole picture), the HDR
## threshold above which a pixel glows.
@export var glow_intensity := 0.8
@export var glow_strength := 1.0
@export var glow_bloom := 0.0
@export var glow_hdr_threshold := 1.0
## The glowcap candle: a cap's lip dims to this share of full and wanders back over about
## this period, each cap its own phase (old `[light] candle_min`, `candle_period_s`).
@export var candle_min := 0.25
@export var candle_period_s := 8.0

# --- haze ---------------------------------------------------------------------------------

## The fog at the back of the world, as a share of the way to the haze colour (the
## palette's `haze_amount`, the old `haze`), reached at the world's back wall.
@export var haze_amount := 0.55
## The fog's curve over the world's depth (Environment.fog_depth_curve: 1 linear).
@export var haze_curve := 1.0

# --- reflections --------------------------------------------------------------------------

## The water's surface while it reflects: roughness, specular and metallic. Physical
## Fresnel at this camera is about 7 %, too weak to read (the old renderer multiplies it
## by `water_reflect` = 6); metallic stands in for that.
@export var water_roughness := 0.05
@export var water_specular := 1.0
@export var water_metallic := 0.35

# --- volumetric fog -----------------------------------------------------------------------

## The fog's density at the world's floor, per voxel of path (old `volumetric_density`), and
## the voxels up over which it thins by e (old `volumetric_falloff`).
@export var volumetric_density := 0.03
@export var volumetric_falloff := 12.0
@export var volumetric_albedo := Color(1.0, 0.95, 0.9)
@export var volumetric_anisotropy := 0.6
## How far the froxel grid reaches (Environment.volumetric_fog_length), in units of the
## camera's distance to the world's back wall.
@export var volumetric_length_share := 1.1

# --- emitter lights -----------------------------------------------------------------------

## Each light's energy and range (voxels: the old glow reaches 3 cells of 4 voxels and fades
## to nothing one cell further), and its attenuation curve.
@export var emitter_light_energy := 1.5
@export var emitter_light_range := 16.0
@export var emitter_light_attenuation := 1.0
## Whether the emitter lights cast shadows; how many of them (the nearest the view's centre)
## do (0: all); cube (0) or dual-paraboloid (1) shadow maps.
@export var emitter_shadows := true
@export var emitter_shadowed_max := 0
@export var emitter_shadow_mode := 0
## Emitters within one cell of this many voxels share one light at their centre, its energy
## their sum (0: one light each).
@export var emitter_group_voxels := 0
## The positional shadow atlas (point and spot shadows): size, texels square, and each
## quadrant's subdivision (Viewport.SHADOW_ATLAS_QUADRANT_SUBDIV_*: 3 is 16 maps, 4 is 64,
## 5 is 256 a quadrant).
@export var positional_atlas_size := 8192
@export var positional_atlas_subdiv := 4

# --- global illumination ------------------------------------------------------------------

## "sdfgi" (Environment.sdfgi) or "voxelgi" (a VoxelGI baked over the world when it is drawn).
@export var gi_mode := "sdfgi"
@export var gi_energy := 1.0
## SDFGI: cascades and the smallest cell (voxels).
@export var sdfgi_cascades := 4
@export var sdfgi_min_cell := 0.5
## VoxelGI: its subdivision (VoxelGI.SUBDIV_*: 0 64, 1 128, 2 256, 3 512 along its longest
## side).
@export var voxelgi_subdiv := 1


## Settings with the effects a flag names on: `all` (every effect but `emitter_lights` and
## `gi`), `none`, or a comma list of switch names (`shadows,ssao`, `all,gi`), and numbers as
## `name=value` (`emitter_group_voxels=8`). Unknown names are refused with an error and ignored.
static func from_list(list: String) -> LightSettings:
	var s := LightSettings.new()
	for raw in list.split(",", false):
		var name := raw.strip_edges()
		if name == "all":
			for k in ALL:
				s.set(k, true)
		elif name == "none" or name == "":
			pass
		elif name in SWITCHES:
			s.set(name, true)
		elif "=" in name:
			var kv := name.split("=")
			var cur = s.get(kv[0])
			if cur == null:
				push_error("LightSettings: no field named " + kv[0])
			elif typeof(cur) == TYPE_STRING:
				s.set(kv[0], kv[1])
			elif typeof(cur) == TYPE_BOOL:
				s.set(kv[0], kv[1] in ["1", "true", "on"])
			elif typeof(cur) == TYPE_INT:
				s.set(kv[0], int(kv[1]))
			else:
				s.set(kv[0], float(kv[1]))
		else:
			push_error("LightSettings: no effect named %s (one of all, none, %s)" % [name, ", ".join(SWITCHES)])
	return s


## The switches that are on, comma separated (`none` for none).
func on_list() -> String:
	var on := []
	for k in SWITCHES:
		if get(k):
			on.append(k)
	if emitter_lights:
		on.append("group=%d shadowed=%s" % [emitter_group_voxels, "all" if emitter_shadowed_max <= 0 else str(emitter_shadowed_max)] if emitter_shadows else "unshadowed")
	if gi:
		on.append(gi_mode)
	return ",".join(on) if on.size() > 0 else "none"
