extends Node3D
## **Emitter lights** (`LightSettings.emitter_lights`): an OmniLight3D on each emitter (the
## glowcap's lips, the bloomcrown's core, the lanternjaw's lure), or one per group of them,
## shadowed as the settings say. Godot's own clustered point lights; nothing of the old
## renderer's glow volume (a voxel flood fill) is ported.
##
## - **Sources**: every live plant or animal whose model emits (`COLOURS`), at the top of its
##   model, in its emission colour. Rebuilt when the feed brings a new frame; placed in the
##   camera's span of the ring like the plants.
## - **Grouping** (`emitter_group_voxels` > 0): sources in one grid cell share a light at
##   their centre, with their summed energy (capped at `GROUP_ENERGY_CAP` sources' worth).
## - **Shadows**: all, or the `emitter_shadowed_max` nearest the view's centre.
## - `stress_total` > 0 (a check, not a setting): that many sources, the real ones repeated
##   with jitter, to measure what thousands of emitters cost.

const COLOURS := {"glowcap": Color("2fb0e2"), "bloomcrown": Color("fed9f2"), "lanternjaw": Color("94f6fd")}
## The light sits this share of the way up the model's height (a cap's lip, a crown's core).
const HEIGHT_SHARE := 0.8
const GROUP_ENERGY_CAP := 4.0

var settings
var stress_total := 0
var dims := Vector3i.ZERO
var lights: Array[OmniLight3D] = []
## [world position (voxels, x unwrapped), colour, weight]
var sources := []
var shadowed := 0
var _placed_at := Vector3(INF, INF, INF)


func rebuild(organisms, s) -> void:
	settings = s
	dims = organisms.dims
	var raw := []
	for b in organisms.plants.values():
		_add(raw, b)
	for b in organisms.animals.values():
		_add(raw, b)
	if stress_total > 0 and raw.size() > 0:
		var rng := RandomNumberGenerator.new()
		rng.seed = 7
		var more := []
		for i in stress_total:
			var r: Array = raw[i % raw.size()]
			var p: Vector3 = r[0]
			if i >= raw.size():
				p += Vector3(rng.randf_range(-6, 6), 0, rng.randf_range(-6, 6))
			more.append([p, r[1], 1.0])
		raw = more
	sources = _grouped(raw, s.emitter_group_voxels)
	_placed_at = Vector3(INF, INF, INF)
	while lights.size() < sources.size():
		var l := OmniLight3D.new()
		add_child(l)
		lights.append(l)
	while lights.size() > sources.size():
		lights.pop_back().queue_free()
	for i in sources.size():
		var l := lights[i]
		l.light_color = sources[i][1]
		l.light_energy = s.emitter_light_energy * minf(sources[i][2], GROUP_ENERGY_CAP)
		l.omni_range = s.emitter_light_range * (1.0 if sources[i][2] <= 1.0 else sqrt(minf(sources[i][2], GROUP_ENERGY_CAP)))
		l.omni_attenuation = s.emitter_light_attenuation
		l.omni_shadow_mode = s.emitter_shadow_mode
		l.shadow_bias = 0.05
		l.shadow_normal_bias = 1.0


func _add(raw: Array, b) -> void:
	if b.leaving or not COLOURS.has(b.model) or b.mesh == null:
		return
	var top: float = b.mesh.get_aabb().end.y
	raw.append([b.at + Vector3(0.0, top * HEIGHT_SHARE, 0.0), COLOURS[b.model], 1.0])


func _grouped(raw: Array, cell: int) -> Array:
	if cell <= 0:
		return raw
	var w := float(dims.x)
	var cells := {}
	for r in raw:
		var p: Vector3 = r[0]
		var key := Vector3i(floori(fposmod(p.x, w) / cell), floori(p.y / cell), floori(p.z / cell))
		if not cells.has(key):
			cells[key] = [Vector3.ZERO, Color(0, 0, 0), 0.0, p.x]
		var c: Array = cells[key]
		var x: float = c[3] + wrapf(p.x - c[3], -w / 2.0, w / 2.0)
		c[0] += Vector3(x, p.y, p.z)
		c[1] += r[1]
		c[2] += 1.0
	var out := []
	for c in cells.values():
		out.append([c[0] / c[2], c[1] / c[2], c[2]])
	return out


## Place the lights in the camera's span of the ring and pick which cast shadows (only after
## a rebuild or when the camera has moved).
func place(cam_x: float, cam_centre: Vector3) -> void:
	if settings == null or cam_centre.is_equal_approx(_placed_at):
		return
	_placed_at = cam_centre
	var w := float(dims.x)
	var lo := cam_x - w / 2.0
	var order := []
	for i in sources.size():
		var p: Vector3 = sources[i][0]
		var x: float = lo + fposmod(p.x - lo, w) if w > 0.0 else p.x
		lights[i].position = Vector3(x, p.y, -p.z)
		order.append([Vector2(x - cam_centre.x, p.y - cam_centre.y).length(), i])
	var cap: int = settings.emitter_shadowed_max
	shadowed = 0
	if settings.emitter_shadows and cap > 0 and cap < sources.size():
		order.sort_custom(func(a, b): return a[0] < b[0])
		for k in order.size():
			lights[order[k][1]].shadow_enabled = k < cap
		shadowed = cap
	else:
		for l in lights:
			l.shadow_enabled = settings.emitter_shadows
		shadowed = sources.size() if settings.emitter_shadows else 0
