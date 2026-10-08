extends Node3D
## The animals and plants of a `WorldView`: baked voxel models placed from the feed,
## smoothed between its frames, faded in and out instead of popping.
##
## - **Animals**: one MeshInstance3D each, the lineage's baked model in the size bin
##   nearest its body (pose models by mobility mode and the hunker flag, from
##   `POSE_MODELS`), else a grey box. Position and heading move linearly from where the
##   body is drawn to the newest frame's over the measured time between frames, so motion
##   is continuous and never jumps a tick; `x` takes the short way round the ring.
## - **Plants**: one MeshInstance3D each, the species' model at the step nearest its growth
##   and its own variant, at the cell above its support face.
## - **Fades**: a body or stand that appears fades in, one that leaves fades out (slower
##   for a death), through the instance's transparency.
##
## Coordinates: the world's voxels `(x, y, z)` are Godot's `(x, y, −z)`; the parent wraps
## each `x` into the camera's span of the ring (`place(cam_x)`).

## Which baked model a lineage wears in a mobility mode (the presenter's `model::mode_model`)
## and when hunkered (`model::hunker_model`). Lineage names are `query("species")`'s, with
## `_` read as `-`. A missing entry wears the lineage's own model.
const POSE_MODELS := {
	"seedporter": {"climb": "seedporter", "perched": "seedporter-stand",
		"ground": "seedporter-ground", "glide": "seedporter-glide"},
}
const HUNKER_MODELS := {"frondgrazer": "frondgrazer-hunker"}
## Seconds to fade a body in, out, and out after a death.
const FADE_IN_S := 0.5
const FADE_OUT_S := 0.5
const FADE_DEATH_S := 1.2
## A frame interval outside this is clamped (seconds): a paused or slow feed still eases.
const MIN_SEGMENT_S := 0.02
const MAX_SEGMENT_S := 0.5
## A body that moves farther than this between frames (voxels) is placed, not slid.
const JUMP_VOXELS := 48.0

var models: VoxelModels
var material: Material
var dims := Vector3i.ZERO
var voxel_m := 0.125
var lineages := {}       # species code -> lineage name
var plant_species := {}  # species code -> species name
var mobility := []       # mobility code -> name
var hunker_bit := 4

## id -> Body
var animals := {}
var plants := {}
var _segment_start := 0.0
var _segment_len := 0.05
var _last_frame_at := -1.0
var _last_tick := -1
var _dying := {}
var _box_mesh: BoxMesh
var _placed_cam_x := INF


class Body:
	var node: MeshInstance3D
	var from := Vector3.ZERO   # world voxels (x unwrapped near `to`)
	var to := Vector3.ZERO
	var from_h := 0.0
	var to_h := 0.0
	var at := Vector3.ZERO     # where it is drawn now, world voxels
	var heading := 0.0
	var model := ""
	var mesh: Mesh
	var alpha := 0.0
	var leaving := false
	var fade_out_s := FADE_OUT_S


func setup(p_models: VoxelModels, p_material: Material, species: Dictionary) -> void:
	models = p_models
	material = p_material
	lineages.clear()
	for a in species.get("animals", []):
		lineages[int(a["id"])] = str(a["name"]).replace("_", "-")
	plant_species.clear()
	for p in species.get("plants", []):
		plant_species[int(p["id"])] = str(p["name"]).replace("_", "-")
	mobility = species.get("mobility", [])
	hunker_bit = int(species.get("flags", {}).get("hunkered", 4))
	_box_mesh = BoxMesh.new()
	_box_mesh.size = Vector3(2, 2, 2)
	var grey := StandardMaterial3D.new()
	grey.albedo_color = Color(0.45, 0.45, 0.5)
	_box_mesh.material = grey


func clear() -> void:
	for b in animals.values():
		b.node.queue_free()
	for b in plants.values():
		b.node.queue_free()
	animals.clear()
	plants.clear()
	_dying.clear()
	_last_tick = -1
	_last_frame_at = -1.0
	_placed_cam_x = INF


func set_world(p_dims: Vector3i, p_voxel_m: float) -> void:
	dims = p_dims
	voxel_m = p_voxel_m if p_voxel_m > 0.0 else 0.125


## Bodies that died since the last frame fade out slowly.
func note_events(events: Array) -> void:
	for e in events:
		if typeof(e) == TYPE_DICTIONARY and str(e.get("kind", "")) == "died":
			_dying[int(e.get("animal", -1))] = true


## Take one feed frame. `instant` places everything without easing or fading (a static
## view). Returns false when the frame is one already taken.
func take_frame(view: Dictionary, now: float, instant: bool) -> bool:
	var tick := int(view.get("tick", -1))
	if tick == _last_tick and not instant:
		return false
	_last_tick = tick
	if _last_frame_at >= 0.0 and not instant:
		var gap := clampf(now - _last_frame_at, MIN_SEGMENT_S, MAX_SEGMENT_S)
		_segment_len = lerpf(_segment_len, gap, 0.3)
	_last_frame_at = now
	_segment_start = now
	_take_animals(view.get("animals", {}), instant)
	_take_plants(view.get("plants", {}), instant)
	_placed_cam_x = INF
	return true


func _take_animals(a: Dictionary, instant: bool) -> void:
	var ids: PackedInt64Array = a.get("ids", PackedInt64Array())
	var species: PackedInt32Array = a.get("species", PackedInt32Array())
	var pos: PackedFloat32Array = a.get("pos", PackedFloat32Array())
	var heading: PackedFloat32Array = a.get("heading", PackedFloat32Array())
	var mob: PackedByteArray = a.get("mobility", PackedByteArray())
	var flags: PackedByteArray = a.get("flags", PackedByteArray())
	var size: PackedFloat32Array = a.get("size", PackedFloat32Array())
	var seen := {}
	var w := float(dims.x)
	for i in ids.size():
		var id := ids[i]
		seen[id] = true
		var p := Vector3(pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]) / voxel_m
		var lineage: String = lineages.get(species[i], "")
		var model := lineage
		var mode: String = mobility[mob[i]] if mob[i] < mobility.size() else ""
		if POSE_MODELS.has(lineage) and POSE_MODELS[lineage].has(mode):
			model = POSE_MODELS[lineage][mode]
		if flags[i] & hunker_bit and HUNKER_MODELS.has(lineage):
			model = HUNKER_MODELS[lineage]
		var b: Body = animals.get(id)
		if b == null:
			b = Body.new()
			b.node = MeshInstance3D.new()
			b.node.material_override = material
			b.node.set_meta("animal_id", id)
			add_child(b.node)
			b.from = p
			b.to = p
			b.at = p
			b.from_h = heading[i]
			b.to_h = heading[i]
			b.heading = heading[i]
			b.alpha = 1.0 if instant else 0.0
			animals[id] = b
		else:
			b.from = b.at
			b.from_h = b.heading
			var to := p
			to.x = b.from.x + wrapf(p.x - b.from.x, -w / 2.0, w / 2.0) if w > 0.0 else p.x
			if instant or b.from.distance_to(to) > JUMP_VOXELS:
				b.from = to
				b.from_h = heading[i]
			b.to = to
			b.to_h = heading[i]
			b.leaving = false
		var mesh = models.animal_mesh(model, size[i]) if model != "" else null
		if mesh == null:
			mesh = _box_mesh
		if mesh != b.mesh:
			b.mesh = mesh
			b.node.mesh = mesh
	for id in animals:
		var b: Body = animals[id]
		if not seen.has(id) and not b.leaving:
			b.leaving = true
			b.from = b.at
			b.to = b.at
			b.fade_out_s = FADE_DEATH_S if _dying.has(id) else FADE_OUT_S
	_dying.clear()


func _take_plants(p: Dictionary, instant: bool) -> void:
	var ids: PackedInt64Array = p.get("ids", PackedInt64Array())
	var species: PackedInt32Array = p.get("species", PackedInt32Array())
	var site: PackedInt32Array = p.get("site", PackedInt32Array())
	var growth: PackedFloat32Array = p.get("growth", PackedFloat32Array())
	var seen := {}
	for i in ids.size():
		var id := ids[i]
		seen[id] = true
		var b: Body = plants.get(id)
		var at := Vector3(site[3 * i] + 0.5, site[3 * i + 1] + 1, site[3 * i + 2] + 0.5)
		if b == null:
			b = Body.new()
			b.node = MeshInstance3D.new()
			b.node.material_override = material
			add_child(b.node)
			b.alpha = 1.0 if instant else 0.0
			plants[id] = b
		b.at = at
		b.leaving = false
		var mesh = models.plant_mesh(plant_species.get(species[i], ""), growth[i], id)
		if mesh == null:
			mesh = _box_mesh
		if mesh != b.mesh:
			b.mesh = mesh
			b.node.mesh = mesh
	for id in plants:
		if not seen.has(id):
			plants[id].leaving = true


## Ease every body toward its newest frame and fade, at `now`.
func advance(now: float, delta: float) -> void:
	var t := clampf((now - _segment_start) / maxf(_segment_len, 0.001), 0.0, 1.0)
	var gone := []
	for id in animals:
		var b: Body = animals[id]
		b.at = b.from.lerp(b.to, t)
		b.heading = lerp_angle(b.from_h, b.to_h, t)
		if _fade(b, delta):
			gone.append(id)
	for id in gone:
		animals[id].node.queue_free()
		animals.erase(id)
	gone.clear()
	for id in plants:
		if _fade(plants[id], delta):
			gone.append(id)
	for id in gone:
		plants[id].node.queue_free()
		plants.erase(id)


## Step a body's fade; true once it has faded out.
func _fade(b: Body, delta: float) -> bool:
	if b.leaving:
		b.alpha = maxf(0.0, b.alpha - delta / b.fade_out_s)
	else:
		b.alpha = minf(1.0, b.alpha + delta / FADE_IN_S)
	b.node.transparency = 1.0 - b.alpha
	return b.leaving and b.alpha <= 0.0


## Put every body where it is drawn, its `x` wrapped into the span `[cam_x − W/2, cam_x +
## W/2)` the camera shows. Plants move only when the camera has.
func place(cam_x: float) -> void:
	var w := float(dims.x)
	var lo := cam_x - w / 2.0
	for b in animals.values():
		var x: float = lo + fposmod(b.at.x - lo, w) if w > 0.0 else b.at.x
		b.node.position = Vector3(x, b.at.y, -b.at.z)
		b.node.rotation = Vector3(0.0, PI / 2.0 - b.heading, 0.0)
	if is_equal_approx(cam_x, _placed_cam_x):
		return
	_placed_cam_x = cam_x
	for b in plants.values():
		var x: float = lo + fposmod(b.at.x - lo, w) if w > 0.0 else b.at.x
		b.node.position = Vector3(x, b.at.y, -b.at.z)


## Where animal `id` is drawn (world voxels), or null.
func animal_at(id: int):
	var b: Body = animals.get(id)
	return null if b == null or b.leaving else b.at


func animal_node(id: int) -> MeshInstance3D:
	var b: Body = animals.get(id)
	return null if b == null else b.node


## Every live animal's id and node.
func live_animals() -> Array:
	var out := []
	for id in animals:
		var b: Body = animals[id]
		if not b.leaving:
			out.append([id, b.node])
	return out
