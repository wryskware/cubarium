extends Node3D
## The light and the rain of a `WorldView`, from the feed's `sky`.
##
## - **Daylight**: the sun's energy follows `daylight`, its height `sun_elevation`, and it
##   swings from one end of the ring to the other over the day (`day_phase`); the ambient
##   light keeps the world readable at night. No shadows, glow or other effects: the plain
##   look of the presenter, lit.
## - **Rain**: streaks falling across the camera's span, as many as the rain mode says
##   (`clear`, `drizzle`, `shower`, `downpour`), in the presenter's rain colour.
## - Everything eases on the frame clock toward the newest sky, so nothing pops.

## The sun's energy at full daylight, and the ambient light's at night and at full day.
const SUN_ENERGY := 0.9
const AMBIENT_NIGHT := 0.35
const AMBIENT_DAY := 0.6
## How far the sun swings along the ring, either side of noon (radians).
const SUN_SWING := 1.0
## Seconds the light and the rain take to ease most of the way to a new sky.
const EASE_S := 1.0
## The share of the rain's streaks each mode shows.
const RAIN_SHARE := {"clear": 0.0, "drizzle": 0.2, "shower": 0.5, "downpour": 1.0}
## Streaks per voxel of the ring's floor at a downpour, and their fall speed (voxels/s).
const RAIN_DENSITY := 0.15
const RAIN_SPEED := 50.0
const RAIN_ALPHA := 0.35

var sun: DirectionalLight3D
var env: Environment
var rain: GPUParticles3D
var dims := Vector3i.ZERO

var _daylight := 1.0
var _elevation := 1.0
var _phase := 0.5
var _rain := 0.0
var _target := {"daylight": 1.0, "elevation": 1.0, "phase": 0.5, "rain": 0.0}


func setup(p_sun: DirectionalLight3D, p_env: Environment, palette: Dictionary) -> void:
	sun = p_sun
	env = p_env
	var t: Dictionary = palette.get("terrain", {})
	# The presenter's sky: its zenith colour at its brightness (linear light).
	var sky := Color.html(str(t.get("sky", "#12093a"))).srgb_to_linear() * float(t.get("sky_brightness", 0.12))
	sky.a = 1.0
	env.background_mode = Environment.BG_COLOR
	env.background_color = sky.linear_to_srgb()
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color.WHITE
	env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	env.glow_enabled = false
	sun.shadow_enabled = false
	sun.light_color = Color.WHITE
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
	rain.amount_ratio = 0.0
	rain.emitting = true
	add_child(rain)


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


## The newest sky. `instant` jumps to it (a static view).
func take_sky(sky: Dictionary, instant: bool) -> void:
	if sky.is_empty():
		return
	_target["daylight"] = float(sky.get("daylight", 1.0))
	_target["elevation"] = float(sky.get("sun_elevation", 1.0))
	_target["phase"] = float(sky.get("day_phase", 0.5))
	var mode := str(sky.get("rain", "clear"))
	var share: float = RAIN_SHARE.get(mode, 0.5 if float(sky.get("rain_m_per_s", 0.0)) > 0.0 else 0.0)
	_target["rain"] = share
	if instant:
		_daylight = _target["daylight"]
		_elevation = _target["elevation"]
		_phase = _target["phase"]
		_rain = share
		_apply()


func advance(delta: float, cam_x: float) -> void:
	var k := 1.0 - exp(-delta / EASE_S)
	_daylight = lerpf(_daylight, _target["daylight"], k)
	_elevation = lerpf(_elevation, _target["elevation"], k)
	_phase = _phase + wrapf(float(_target["phase"]) - _phase, -0.5, 0.5) * k
	_rain = lerpf(_rain, _target["rain"], k)
	_apply()
	if dims.x > 0:
		rain.position = Vector3(cam_x, float(dims.y) + 4.0, -float(dims.z) / 2.0)


func _apply() -> void:
	var e := clampf(_elevation, 0.05, PI / 2.0)
	var swing := (fposmod(_phase, 1.0) - 0.5) * 2.0 * SUN_SWING
	# Toward the sun: up by its elevation, along the ring by the hour, from the viewer's side.
	var toward := Vector3(sin(swing) * cos(e), sin(e), cos(swing) * cos(e)).normalized()
	sun.look_at_from_position(Vector3.ZERO, -toward, Vector3.UP if absf(toward.y) < 0.99 else Vector3.BACK)
	sun.light_energy = SUN_ENERGY * clampf(_daylight, 0.0, 1.0)
	env.ambient_light_energy = lerpf(AMBIENT_NIGHT, AMBIENT_DAY, clampf(_daylight, 0.0, 1.0))
	rain.amount_ratio = clampf(_rain, 0.0, 1.0)
