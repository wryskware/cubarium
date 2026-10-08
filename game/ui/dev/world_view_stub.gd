extends Control
## **A stand-in for package B's `WorldView`**, with the same API, so the shell runs before
## B lands. Not the game's look: a flat front elevation of the world (the first cell along
## the depth that holds rock, soil or free water, darker the further back it is), plants
## as dots and animals as small squares. The shell uses B's scene instead the moment
## `res://world/world_view.tscn` exists (`res://ui/world_views.gd`).
##
## API (the contract in design/handoffs/godot-direction-2026-10-07.md):
## `attach(session)`, `detach()`, `show_view(view)`, `input_enabled`, `reset_framing()`,
## `animal_hovered(id)` / `animal_clicked(id)`, `set_highlight(id)`, `follow(id)`,
## `stop_follow()`, `capture(size) -> Image`.

signal animal_hovered(id: int)
signal animal_clicked(id: int)

const SKY := Color(0.05, 0.06, 0.085)
const MATERIALS := [Color(0, 0, 0, 0), Color(0.2, 0.19, 0.21), Color(0.46, 0.45, 0.43), Color(0.43, 0.31, 0.19)]
const WATER := Color(0.22, 0.45, 0.75)
const PLANT := Color(0.35, 0.62, 0.3)
## Dev colours for the lineages by id: distinguishable, nothing more.
const ANIMALS := [
	Color(0.93, 0.85, 0.55), Color(0.62, 0.86, 0.5), Color(0.95, 0.5, 0.4),
	Color(0.75, 0.62, 0.95), Color(0.5, 0.85, 0.85), Color(0.9, 0.7, 0.9),
	Color(1.0, 0.95, 0.4), Color(0.95, 0.65, 0.3), Color(0.85, 0.85, 0.95),
]
const PICK_PX := 12.0
const CLICK_SLOP_PX := 5.0
const COLUMNS_PER_FRAME := 48
const PAN_SPEED := 1.2  # screen widths per second under keys or the stick
const MAX_ZOOM := 8.0

## When false, pointer and pan input are ignored (the shell turns it off under a menu).
var input_enabled := true:
	set(v):
		input_enabled = v
		if not v:
			_dragging = false
			_set_hovered(-1)

var _session = null
var _generation := -1
var _dims := Vector3i.ZERO
var _chunk_dims := Vector3i.ZERO
var _cs := 16
var _voxel_m := 0.125
var _chunks := {}  # chunk index -> PackedByteArray (material, free water, pore water)
var _empty := {}  # chunk index -> true when it holds no material and no free water
var _dirty := {}  # chunk column (cx + cy * 65536) -> true
var _image: Image
var _texture: ImageTexture
var _animals := {}
var _plants := {}
var _hovered := -1
var highlighted := -1
var following := -1
var _zoom := 1.0
var _centre := Vector2.ZERO
var _shown := Vector2.ZERO
var _dragging := false
## The animals' live settings asked for, [movement, subpixel, startup] (the real view's API).
var motion := []
var _press := Vector2.ZERO
var _moved := false


func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_STOP
	clip_contents = true
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST


# --- The contract -------------------------------------------------------------------

func attach(session) -> void:
	_session = session
	_generation = -1


func detach() -> void:
	_session = null


func show_view(view: Dictionary) -> void:
	_session = null
	_generation = -1
	_apply(view)


func reset_framing() -> void:
	_zoom = 1.0
	_centre = Vector2(_dims.x, _dims.y) * 0.5
	_shown = _centre


func set_highlight(id: int) -> void:
	highlighted = id
	queue_redraw()


func set_animal_motion(movement: String, subpixel: bool, startup := false) -> void:
	if startup and not motion.is_empty() and not motion[2]:
		return
	motion = [movement, subpixel, startup]


## The stub draws no animals' motion: what was asked, as the real view's renderer would say.
func animal_motion() -> Dictionary:
	if motion.is_empty():
		return {}
	return {"models": true, "movement": motion[0], "subpixel": motion[1],
		"drawn_movement": motion[0], "drawn_subpixel": motion[1],
		"env_movement": false, "env_subpixel": false}


func follow(id: int) -> void:
	following = id


func stop_follow() -> void:
	following = -1


func capture(size: Vector2i) -> Image:
	_rebuild(1 << 30)
	var out := Image.create_empty(maxi(size.x, 1), maxi(size.y, 1), false, Image.FORMAT_RGBA8)
	out.fill(SKY)
	if _image == null:
		return out
	var k := minf(float(size.x) / _dims.x, float(size.y) / _dims.y)
	var w := maxi(int(_dims.x * k), 1)
	var h := maxi(int(_dims.y * k), 1)
	var scaled := _image.duplicate() as Image
	scaled.resize(w, h, Image.INTERPOLATE_BILINEAR)
	var under := Image.create_empty(w, h, false, Image.FORMAT_RGBA8)
	under.fill(SKY)
	under.blend_rect(scaled, Rect2i(0, 0, w, h), Vector2i.ZERO)
	out.blit_rect(under, Rect2i(0, 0, w, h), Vector2i((size.x - w) / 2, (size.y - h) / 2))
	return out


# --- Feeding --------------------------------------------------------------------------

func _process(delta: float) -> void:
	if _session != null:
		var view: Dictionary = _session.take_view()
		_session.take_events()
		_apply(view)
	_rebuild(COLUMNS_PER_FRAME)
	_pan(delta)
	queue_redraw()


func _apply(view: Dictionary) -> void:
	if view.is_empty() or not view.has("dims"):
		return
	var dims: Vector3i = view["dims"]
	if dims.x <= 0 or dims.y <= 0 or dims.z <= 0:
		return
	var gen: int = view.get("generation", 0)
	if gen != _generation or view.get("full", false):
		var fresh: bool = view["dims"] != _dims
		_generation = gen
		_dims = view["dims"]
		_chunk_dims = view["chunk_dims"]
		_cs = view.get("chunk_size", 16)
		_voxel_m = view.get("voxel_m", 0.125)
		_chunks.clear()
		_empty.clear()
		_dirty.clear()
		_image = Image.create_empty(_dims.x, _dims.y, false, Image.FORMAT_RGBA8)
		_image.fill(Color(0, 0, 0, 0))
		_texture = ImageTexture.create_from_image(_image)
		if fresh:
			reset_framing()
	var coords: PackedInt32Array = view.get("chunk_coords", PackedInt32Array())
	var voxels: PackedByteArray = view.get("voxels", PackedByteArray())
	var bytes := _cs * _cs * _cs * 3
	var plane := _cs * _cs * _cs
	for n in coords.size() / 3:
		var cx := coords[n * 3]
		var cy := coords[n * 3 + 1]
		var cz := coords[n * 3 + 2]
		var key := (cz * _chunk_dims.y + cy) * _chunk_dims.x + cx
		var data := voxels.slice(n * bytes, (n + 1) * bytes)
		_chunks[key] = data
		var empty := data.slice(0, plane).count(0) == plane and data.slice(plane, plane * 2).count(0) == plane
		if empty:
			_empty[key] = true
		else:
			_empty.erase(key)
		_dirty[cx + cy * 65536] = true
	if view.has("animals"):
		_animals = view["animals"]
	if view.has("plants"):
		_plants = view["plants"]


## Redraw up to `limit` dirty chunk columns of the elevation image.
func _rebuild(limit: int) -> void:
	if _dirty.is_empty() or _image == null:
		return
	var done := 0
	for col in _dirty.keys():
		_dirty.erase(col)
		_column(col % 65536, col / 65536)
		done += 1
		if done >= limit:
			break
	_texture.update(_image)


func _column(cx: int, cy: int) -> void:
	var cs := _cs
	var plane := cs * cs * cs
	for ly in cs:
		var y := cy * cs + ly
		if y >= _dims.y:
			break
		for lx in cs:
			var x := cx * cs + lx
			if x >= _dims.x:
				break
			var colour := Color(0, 0, 0, 0)
			var water_z := -1
			var found := false
			for cz in _chunk_dims.z:
				var key := (cz * _chunk_dims.y + cy) * _chunk_dims.x + cx
				if _empty.has(key) or not _chunks.has(key):
					continue
				var data: PackedByteArray = _chunks[key]
				for lz in cs:
					var i := (lz * cs + ly) * cs + lx
					var m := data[i]
					if water_z < 0 and data[plane + i] > 0:
						water_z = cz * cs + lz
					if m != 0:
						var z := cz * cs + lz
						var shade := 1.0 - 0.6 * float(z) / maxf(_dims.z, 1)
						colour = MATERIALS[mini(m, 3)] * shade
						colour.a = 1.0
						found = true
						break
				if found:
					break
			if water_z >= 0:
				colour = colour.lerp(WATER, 0.65) if found else WATER
				colour.a = 1.0
			_image.set_pixel(x, _dims.y - 1 - y, colour)


# --- Framing and input --------------------------------------------------------------

func _scale() -> float:
	if _dims.x == 0:
		return 1.0
	return minf(size.x / _dims.x, size.y / _dims.y) * _zoom


func _to_screen(p: Vector2) -> Vector2:
	return size * 0.5 + (p - _shown) * _scale()


func _clamped(c: Vector2) -> Vector2:
	var half := size * 0.5 / _scale()
	var out := c
	out.x = _dims.x * 0.5 if half.x * 2.0 >= _dims.x else clampf(c.x, half.x, _dims.x - half.x)
	out.y = _dims.y * 0.5 if half.y * 2.0 >= _dims.y else clampf(c.y, half.y, _dims.y - half.y)
	return out


func _pan(delta: float) -> void:
	if following >= 0:
		var p := _animal_px(following)
		if p.x >= 0.0:
			_centre = p
	if input_enabled and following < 0:
		var v := Vector2(
			Input.get_joy_axis(0, JOY_AXIS_RIGHT_X) if absf(Input.get_joy_axis(0, JOY_AXIS_RIGHT_X)) > 0.2 else 0.0,
			Input.get_joy_axis(0, JOY_AXIS_RIGHT_Y) if absf(Input.get_joy_axis(0, JOY_AXIS_RIGHT_Y)) > 0.2 else 0.0)
		if Input.is_physical_key_pressed(KEY_A):
			v.x -= 1.0
		if Input.is_physical_key_pressed(KEY_D):
			v.x += 1.0
		if Input.is_physical_key_pressed(KEY_W):
			v.y -= 1.0
		if Input.is_physical_key_pressed(KEY_S):
			v.y += 1.0
		if v != Vector2.ZERO:
			_centre += v * PAN_SPEED * delta * size.x / _scale()
	_centre = _clamped(_centre)
	_shown = _clamped(_shown.lerp(_centre, 1.0 - exp(-12.0 * delta)))


func _gui_input(event: InputEvent) -> void:
	if not input_enabled:
		return
	if event is InputEventMouseButton:
		var b := event as InputEventMouseButton
		if b.button_index == MOUSE_BUTTON_LEFT:
			if b.pressed:
				_dragging = true
				_moved = false
				_press = b.position
			else:
				_dragging = false
				if not _moved:
					var id := _pick(b.position)
					if id >= 0:
						animal_clicked.emit(id)
			accept_event()
		elif b.pressed and b.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
			_zoom = clampf(_zoom * (1.15 if b.button_index == MOUSE_BUTTON_WHEEL_UP else 1.0 / 1.15), 1.0, MAX_ZOOM)
			accept_event()
	elif event is InputEventMouseMotion:
		var m := event as InputEventMouseMotion
		if _dragging:
			if m.position.distance_to(_press) > CLICK_SLOP_PX:
				_moved = true
			if _moved:
				_centre -= m.relative / _scale()
				_shown = _clamped(_shown - m.relative / _scale())
				stop_follow()
		else:
			_set_hovered(_pick(m.position))


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT:
		_set_hovered(-1)


func _set_hovered(id: int) -> void:
	if id != _hovered:
		_hovered = id
		animal_hovered.emit(id)


func _animal_px(id: int) -> Vector2:
	var ids: PackedInt64Array = _animals.get("ids", PackedInt64Array())
	var at := ids.find(id)
	if at < 0:
		return Vector2(-1, -1)
	var pos: PackedFloat32Array = _animals["pos"]
	return Vector2(pos[at * 3] / _voxel_m, _dims.y - pos[at * 3 + 1] / _voxel_m)


func _pick(screen: Vector2) -> int:
	var ids = _animals.get("ids", [])
	if ids.is_empty():
		return -1
	var pos = _animals["pos"]
	var best := -1
	var best_d := PICK_PX
	for i in ids.size():
		var p := _to_screen(Vector2(pos[i * 3] / _voxel_m, _dims.y - pos[i * 3 + 1] / _voxel_m))
		var d := p.distance_to(screen)
		if d < best_d:
			best_d = d
			best = ids[i]
	return best


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), SKY)
	if _texture == null:
		return
	var s := _scale()
	draw_texture_rect(_texture, Rect2(_to_screen(Vector2.ZERO), Vector2(_dims.x, _dims.y) * s), false)
	var dot := maxf(s, 2.0)
	var sites = _plants.get("site", [])
	for i in sites.size() / 3:
		var p := _to_screen(Vector2(sites[i * 3] + 0.5, _dims.y - sites[i * 3 + 1] - 0.5))
		draw_rect(Rect2(p - Vector2(dot, dot) * 0.5, Vector2(dot, dot)), PLANT)
	var ids = _animals.get("ids", [])
	if ids.is_empty():
		return
	var pos = _animals["pos"]
	var species = _animals.get("species", [])
	var body := maxf(s * 2.0, 4.0)
	for i in ids.size():
		var p := _to_screen(Vector2(pos[i * 3] / _voxel_m, _dims.y - pos[i * 3 + 1] / _voxel_m))
		var sp: int = species[i] if i < species.size() else 0
		draw_rect(Rect2(p - Vector2(body, body) * 0.5, Vector2(body, body)), ANIMALS[sp % ANIMALS.size()])
		if ids[i] == highlighted:
			draw_rect(Rect2(p - Vector2(body, body), Vector2(body, body) * 2.0), Color.WHITE, false, 2.0)
		elif ids[i] == _hovered:
			draw_rect(Rect2(p - Vector2(body, body) * 0.8, Vector2(body, body) * 1.6), Color(1, 1, 1, 0.6), false, 1.0)
