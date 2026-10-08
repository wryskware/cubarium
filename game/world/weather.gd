extends Node3D
## The light and the rain of a `WorldView`, from the feed's `sky`, by its `LightSettings`.
##
## - **The sun**: placed as the old renderer's lit tier places it (`crates/cubarium-gpu/src/
##   weather.rs`, `look`): the configured noon sun, swung toward its rising and setting
##   azimuths over the day, its elevation the day clock's scaled to the noon sun's and held
##   above a floor; it fades in over the first degrees above the horizon, dims under cloud
##   and warms as it sinks. At night the **moon** takes its place opposite, weaker.
## - **Ambient light**: by day and night (`ambient_*`), dimmed under cloud.
## - **Effects** (each its own switch in `LightSettings`, off by default): shadows, SSAO,
##   SSIL, the sky gradient, glow (the environment's half; the emitters' material is the
##   view's), depth haze, the sky's reflection, volumetric fog.
## - **Rain**: streaks falling across the camera's span, as many as the rain mode says
##   (`clear`, `drizzle`, `shower`, `downpour`), in the presenter's rain colour.
## - Everything eases on the frame clock toward the newest sky, so nothing pops.

## Seconds the light and the rain take to ease most of the way to a new sky.
const EASE_S := 1.0
## The share of the rain's streaks each mode shows.
const RAIN_SHARE := {"clear": 0.0, "drizzle": 0.2, "shower": 0.5, "downpour": 1.0}
## Streaks per voxel of the ring's floor at a downpour, and their fall speed (voxels/s).
const RAIN_DENSITY := 0.15
const RAIN_SPEED := 50.0
const RAIN_ALPHA := 0.35
## How far behind the camera's near plane the sky gradient sits, as a share of `far`.
const SKY_AT := 0.95
## The sky's radiance (for reflections) is recomputed only when its colour has moved this
## far (it costs a radiance bake).
const SKY_RADIANCE_STEP := 0.03

const SKY_SHADER := """
shader_type spatial;
render_mode unshaded, fog_disabled, cull_disabled, shadows_disabled;
uniform vec3 top : source_color;
uniform vec3 middle : source_color;
uniform vec3 bottom : source_color;
void fragment() {
	float v = UV.y;
	ALBEDO = v < 0.5 ? mix(top, middle, v * 2.0) : mix(middle, bottom, v * 2.0 - 1.0);
}
"""

var sun: DirectionalLight3D
var env: Environment
var camera: Camera3D
var rain: GPUParticles3D
var dims := Vector3i.ZERO
var settings: LightSettings
var sky_quad: MeshInstance3D
var _sky_mat: ShaderMaterial
var _sky_res: Sky
var _sky_proc: ProceduralSkyMaterial
var _plain_background := Color.BLACK
var _haze_colour := Color.BLACK
var _radiance_day := -1.0
var fog_volume: FogVolume
var _fog_mat: FogMaterial
## VoxelGI: one node over each terrain copy, sharing one bake.
var voxel_gi: Array[VoxelGI] = []
var gi_bake_ms := 0.0

var _daylight := 1.0
var _elevation := 1.0
var _phase := 0.5
var _rain := 0.0
var _cloud := 0.0
var _target := {"daylight": 1.0, "elevation": 1.0, "phase": 0.5, "rain": 0.0, "cloud": 0.0}


func setup(p_sun: DirectionalLight3D, p_env: Environment, palette: Dictionary, p_camera: Camera3D) -> void:
	sun = p_sun
	env = p_env
	camera = p_camera
	var t: Dictionary = palette.get("terrain", {})
	# The presenter's sky: its zenith colour at its brightness (linear light).
	var sky := Color.html(str(t.get("sky", "#12093a"))).srgb_to_linear() * float(t.get("sky_brightness", 0.12))
	sky.a = 1.0
	_plain_background = sky.linear_to_srgb()
	_haze_colour = Color.html(str(t.get("haze", "#241657")))
	env.background_mode = Environment.BG_COLOR
	env.background_color = _plain_background
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color.WHITE
	env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	env.glow_enabled = false
	sun.shadow_enabled = false
	sun.light_color = Color.WHITE
	_build_sky()
	_build_rain(palette)
	fog_volume = FogVolume.new()
	fog_volume.name = "Air"
	fog_volume.shape = RenderingServer.FOG_VOLUME_SHAPE_BOX
	_fog_mat = FogMaterial.new()
	fog_volume.material = _fog_mat
	fog_volume.visible = false
	add_child(fog_volume)
	apply_settings(LightSettings.new())


func _build_sky() -> void:
	sky_quad = MeshInstance3D.new()
	sky_quad.name = "Sky"
	var q := QuadMesh.new()
	q.size = Vector2(1, 1)
	_sky_mat = ShaderMaterial.new()
	var sh := Shader.new()
	sh.code = SKY_SHADER
	_sky_mat.shader = sh
	q.material = _sky_mat
	sky_quad.mesh = q
	sky_quad.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	sky_quad.visible = false
	camera.add_child(sky_quad)
	# The sky as radiance for reflections (an orthographic view sees one direction of a
	# sky, so the picture's own sky is the quad above; this only lights).
	_sky_proc = ProceduralSkyMaterial.new()
	_sky_proc.sun_angle_max = 0.0
	_sky_res = Sky.new()
	_sky_res.sky_material = _sky_proc
	_sky_res.radiance_size = Sky.RADIANCE_SIZE_64


func _build_rain(palette: Dictionary) -> void:
	rain = GPUParticles3D.new()
	rain.name = "Rain"
	var pm := ParticleProcessMaterial.new()
	pm.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	pm.direction = Vector3.DOWN
	pm.spread = 0.0
	pm.initial_velocity_min = RAIN_SPEED
	pm.initial_velocity_max = RAIN_SPEED * 1.2
	pm.gravity = Vector3.ZERO
	rain.process_material = pm
	var streak := QuadMesh.new()
	streak.size = Vector2(0.12, 1.6)
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.billboard_mode = BaseMaterial3D.BILLBOARD_FIXED_Y
	m.billboard_keep_scale = true
	var rc := Color.html(str(palette.get("rain", "#b8f0ff")))
	rc.a = RAIN_ALPHA
	m.albedo_color = rc
	streak.material = m
	rain.draw_pass_1 = streak
	rain.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	rain.amount_ratio = 0.0
	rain.emitting = true
	add_child(rain)


## Apply `s`: the environment's effects and the sun's shadow. Cheap; call again on a change.
func apply_settings(s: LightSettings) -> void:
	settings = s
	env.tonemap_mode = s.tonemap
	env.tonemap_exposure = s.exposure
	# Shadows: one orthogonal split over the world (the camera is orthographic, so the
	# parallel splits only spend resolution on depth the view does not have).
	sun.shadow_enabled = s.shadows
	sun.directional_shadow_mode = DirectionalLight3D.SHADOW_ORTHOGONAL
	sun.shadow_bias = s.shadow_bias
	sun.shadow_normal_bias = s.shadow_normal_bias
	sun.shadow_blur = s.shadow_blur
	sun.shadow_opacity = s.shadow_opacity
	if s.shadows:
		RenderingServer.directional_shadow_atlas_set_size(s.shadow_map_size, true)
	env.ssao_enabled = s.ssao
	env.ssao_radius = s.ssao_radius
	env.ssao_intensity = s.ssao_intensity
	env.ssao_power = s.ssao_power
	env.ssil_enabled = s.ssil
	env.ssil_radius = s.ssil_radius
	env.ssil_intensity = s.ssil_intensity
	env.glow_enabled = s.glow
	env.glow_intensity = s.glow_intensity
	env.glow_strength = s.glow_strength
	env.glow_bloom = s.glow_bloom
	env.glow_hdr_threshold = s.glow_hdr_threshold
	sky_quad.visible = s.sky
	if s.reflections or s.gi:
		env.sky = _sky_res
		_radiance_day = -1.0
	else:
		env.sky = null
	env.reflected_light_source = Environment.REFLECTION_SOURCE_SKY if s.reflections else Environment.REFLECTION_SOURCE_DISABLED
	# Under GI the ambient light comes from the sky, so SDFGI and VoxelGI can occlude it in
	# caves and under overhangs (a constant ambient colour lights every face alike).
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY if s.gi else Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_sky_contribution = 1.0
	env.fog_enabled = s.haze
	env.fog_mode = Environment.FOG_MODE_DEPTH
	env.fog_light_color = _haze_colour
	env.fog_density = s.haze_amount
	env.fog_depth_curve = s.haze_curve
	env.fog_sun_scatter = 0.0
	env.fog_sky_affect = 0.0
	# Volumetric fog only inside the world's box (a FogVolume): the global density is 0, or
	# the hundreds of voxels of empty air between the orthographic camera and the world
	# would fog everything.
	env.volumetric_fog_enabled = s.volumetric
	env.volumetric_fog_density = 0.0
	env.volumetric_fog_albedo = s.volumetric_albedo
	env.volumetric_fog_anisotropy = s.volumetric_anisotropy
	env.volumetric_fog_sky_affect = 0.0
	fog_volume.visible = s.volumetric
	_fog_mat.density = s.volumetric_density
	_fog_mat.albedo = s.volumetric_albedo
	_fog_mat.height_falloff = 1.0 / maxf(s.volumetric_falloff, 0.1)
	# Global illumination.
	env.sdfgi_enabled = s.gi and s.gi_mode == "sdfgi"
	env.sdfgi_cascades = s.sdfgi_cascades
	env.sdfgi_min_cell_size = s.sdfgi_min_cell
	env.sdfgi_energy = s.gi_energy
	env.sdfgi_use_occlusion = true
	for g in voxel_gi:
		g.visible = s.gi and s.gi_mode == "voxelgi"
	_fit()
	_apply()


func set_world(p_dims: Vector3i) -> void:
	dims = p_dims
	if dims.x <= 0:
		return
	var w := float(dims.x)
	var h := float(dims.y)
	var d := float(dims.z)
	var pm: ParticleProcessMaterial = rain.process_material
	pm.emission_box_extents = Vector3(w / 2.0, 0.5, d / 2.0)
	rain.amount = clampi(int(w * d * RAIN_DENSITY), 200, 12000)
	rain.lifetime = (h + 8.0) / RAIN_SPEED
	rain.visibility_aabb = AABB(Vector3(-w / 2.0 - 8.0, -h - 16.0, -d / 2.0 - 8.0), Vector3(w + 16.0, h + 32.0, d + 16.0))
	_fit()


## Size what depends on the world and the camera: the shadow's reach, the haze's depth
## range, the volumetric fog's length, the sky quad. Call when the camera's framing moves
## its distance or the viewport resizes.
func _fit() -> void:
	if dims.x <= 0 or camera == null:
		return
	var w := float(dims.x)
	var t := deg_to_rad(30.0)
	# The camera sits `w + h + d` back from the world's front centre line (WorldView).
	var back := w + dims.y + dims.z
	sun.directional_shadow_max_distance = back + w + dims.z
	# The haze: none at the world's front edge, `haze_amount` at its back wall, by depth
	# along the view (the front ground is about `back` away, the back wall `d·cos t` more).
	env.fog_depth_begin = back - dims.y * sin(t)
	env.fog_depth_end = back + dims.z * cos(t) + dims.y * sin(t)
	env.volumetric_fog_length = (back + dims.z + dims.y) * (settings.volumetric_length_share if settings else 1.1)
	var vp := camera.get_viewport().get_visible_rect().size if camera.is_inside_tree() else Vector2(16, 9)
	var aspect := vp.y / maxf(vp.x, 1.0)
	sky_quad.position = Vector3(0, 0, -camera.far * SKY_AT)
	sky_quad.scale = Vector3(w * 1.2, w * aspect * 1.2, 1.0)


## The newest sky. `instant` jumps to it (a static view).
func take_sky(sky: Dictionary, instant: bool) -> void:
	if sky.is_empty():
		return
	_target["daylight"] = float(sky.get("daylight", 1.0))
	_target["elevation"] = float(sky.get("sun_elevation", 1.0))
	_target["phase"] = float(sky.get("day_phase", 0.5))
	_target["cloud"] = float(sky.get("cloud_cover", 0.0))
	var mode := str(sky.get("rain", "clear"))
	var share: float = RAIN_SHARE.get(mode, 0.5 if float(sky.get("rain_m_per_s", 0.0)) > 0.0 else 0.0)
	_target["rain"] = share
	if instant:
		_daylight = _target["daylight"]
		_elevation = _target["elevation"]
		_phase = _target["phase"]
		_cloud = _target["cloud"]
		_rain = share
		_apply()


func advance(delta: float, cam_x: float) -> void:
	var k := 1.0 - exp(-delta / EASE_S)
	_daylight = lerpf(_daylight, _target["daylight"], k)
	_elevation = lerpf(_elevation, _target["elevation"], k)
	_phase = _phase + wrapf(float(_target["phase"]) - _phase, -0.5, 0.5) * k
	_rain = lerpf(_rain, _target["rain"], k)
	_cloud = lerpf(_cloud, _target["cloud"], k)
	_fit()
	_apply()
	if dims.x > 0:
		rain.position = Vector3(cam_x, float(dims.y) + 4.0, -float(dims.z) / 2.0)
		fog_volume.position = Vector3(cam_x, float(dims.y) / 2.0, -float(dims.z) / 2.0)
		fog_volume.size = Vector3(float(dims.x) + 32.0, float(dims.y), float(dims.z))


## Toward the sun (Godot axes) for a day phase and the clock's elevation (radians), as the
## old lit tier's `place`.
func _toward(phase: float, elevation: float) -> Vector3:
	var s := settings
	var n := s.noon_sun.normalized()
	var noon_el := asin(clampf(n.y, -1.0, 1.0))
	var az0 := atan2(n.x, -n.z)
	var from_noon := fposmod(phase, 1.0) - 0.5
	var u := minf(absf(from_noon) / maxf(s.azimuth_swing, 1e-3), 1.0)
	var end := deg_to_rad(s.rise_azimuth_deg if from_noon < 0.0 else s.set_azimuth_deg)
	var az := lerpf(az0, end, u)
	var el := clampf(elevation * noon_el / deg_to_rad(s.noon_elevation_deg),
		minf(deg_to_rad(s.min_elevation_deg), noon_el), noon_el)
	# The old axes' z runs into the scene; Godot's into the scene is −z.
	return Vector3(sin(az) * cos(el), sin(el), cos(az) * cos(el))


func _apply() -> void:
	if settings == null:
		return
	var s := settings
	var dl := clampf(_daylight, 0.0, 1.0)
	var cloud := clampf(_cloud, 0.0, 1.0)
	var el_deg := rad_to_deg(_elevation)
	var clouded := 1.0 - s.overcast_sun * pow(cloud, 1.5)
	var toward: Vector3
	var strength: float
	var colour := Color.WHITE
	if _elevation >= 0.0:
		toward = _toward(_phase, _elevation)
		strength = smoothstep(0.0, s.sun_fade_deg, el_deg) * clouded
		var warm := smoothstep(30.0, 2.0, el_deg)
		colour = Color.WHITE.lerp(s.sun_warm_colour, s.sun_warm_share * warm)
	else:
		toward = _toward(_phase + 0.5, -_elevation)
		strength = s.moon_share * smoothstep(0.0, s.sun_fade_deg, -el_deg) * clouded
		colour = Color.WHITE.lerp(s.ambient_night_colour, 0.3)
	sun.look_at_from_position(Vector3.ZERO, -toward, Vector3.UP if absf(toward.y) < 0.99 else Vector3.BACK)
	sun.light_energy = s.sun_energy * strength
	sun.light_color = colour
	sun.visible = strength > 0.0
	env.ambient_light_energy = s.ambient_energy * lerpf(s.ambient_night_share, 1.0, dl) * (1.0 - s.overcast_ambient * cloud)
	env.ambient_light_color = s.ambient_night_colour.lerp(s.ambient_day_colour, dl)
	rain.amount_ratio = clampf(_rain, 0.0, 1.0)
	# The sky: the day gradient comes in as the sun climbs, over the night's.
	var day := smoothstep(s.sky_day_from_deg, s.sky_day_to_deg, el_deg) * dl if _elevation > 0.0 else 0.0
	var top := s.sky_night_top.lerp(s.sky_day_top, day)
	var middle := s.sky_night_top.lerp(s.sky_night_bottom, 0.5).lerp(s.sky_day_middle, day)
	var bottom := s.sky_night_bottom.lerp(s.sky_day_bottom, day)
	if s.sky:
		_sky_mat.set_shader_parameter("top", top)
		_sky_mat.set_shader_parameter("middle", middle)
		_sky_mat.set_shader_parameter("bottom", bottom)
	if (s.reflections or s.gi) and absf(day - _radiance_day) > SKY_RADIANCE_STEP:
		_radiance_day = day
		_sky_proc.sky_top_color = top
		_sky_proc.sky_horizon_color = middle
		_sky_proc.ground_horizon_color = bottom
		_sky_proc.ground_bottom_color = bottom
	if s.haze:
		env.fog_light_color = _haze_colour * lerpf(0.35, 1.0, dl)


## Bake VoxelGI over the drawn terrain (`copies`: the terrain's copy nodes, one VoxelGI over
## each, sharing the first one's bake). Synchronous; its time is `gi_bake_ms`.
func bake_voxel_gi(copies: Array) -> void:
	for g in voxel_gi:
		g.queue_free()
	voxel_gi.clear()
	if dims.x <= 0:
		return
	var started := Time.get_ticks_usec()
	var size := Vector3(dims.x, dims.y, dims.z)
	for i in copies.size():
		var g := VoxelGI.new()
		g.name = "GI%d" % i
		g.subdiv = settings.voxelgi_subdiv
		g.size = size
		copies[i].add_child(g)
		g.position = Vector3(size.x / 2.0, size.y / 2.0, -size.z / 2.0)
		voxel_gi.append(g)
	voxel_gi[0].bake(copies[0])
	if voxel_gi[0].data:
		voxel_gi[0].data.energy = settings.gi_energy
	for i in range(1, voxel_gi.size()):
		voxel_gi[i].data = voxel_gi[0].data
	gi_bake_ms = (Time.get_ticks_usec() - started) / 1000.0
	print("weather: VoxelGI baked in %.0f ms" % gi_bake_ms)
