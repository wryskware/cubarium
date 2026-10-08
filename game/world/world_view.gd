class_name WorldView
extends Node3D
## **The development presentation of a world** (package B, `design/handoffs/godot-direction-
## 2026-10-07.md`): terrain and water from the feed's chunks, the organisms as their baked
## voxel models, daylight and rain, an elevated orthographic camera that pans, and picking.
##
## API (what the shell codes against):
## - `attach(session)` draws a live `CubariumSession`, taking `take_view()` and
##   `take_events()` every frame; `detach()` stops (the drawn world stays, and the session
##   keeps the changes for the next `attach`). While attached, nothing else should take
##   views or events from that session.
## - `show_view(view)` draws one whole static view (a candidate's `candidate_view(i)`).
## - `input_enabled`: false ignores pointer and pan input (a menu is open).
## - `reset_framing()`: back to the default framing, the world's whole width in view.
## - Signals `animal_hovered(id)` (−1 for none) and `animal_clicked(id)` (−1 for empty
##   ground). A click is a press and release without dragging; a drag pans.
## - `set_highlight(id)` tints one animal (−1 clears); `follow(id)` keeps it in view,
##   easing; `stop_follow()`. Panning by hand stops following.
## - `capture(size) -> Image`: the current framing, rendered at `size` without the UI.
##
## Panning: mouse drag, arrows/WASD, and a gamepad's left stick. The ring has no ends, so
## `x` pans round it without limit (the seam moves); up and down are clamped to the world's
## edges, and only move when the world is taller than the view. Every pan eases.
##
## Debug lines: `perf_line()` (fps, ms, mesh cost) for the dev scene.

signal animal_hovered(id: int)
signal animal_clicked(id: int)

const PALETTE_PATH := "res://world/data/palette.json"
## The baked models, relative to the project: `assets/voxel-models/<voxel mm>/`.
const MODELS_ROOT := "../assets/voxel-models"
## The camera's elevation (the presenter's `tilt_degrees`).
const TILT_DEG := 30.0
## Seconds the camera takes to ease most of the way to where it is going, by hand and when
## following.
const PAN_EASE_S := 0.12
const DRAG_EASE_S := 0.05
const FOLLOW_EASE_S := 0.35
## Key and stick pan speed: view widths per second.
const PAN_SPEED := 0.5
const STICK_DEADZONE := 0.2
## A press that moves less than this many pixels before release is a click.
const CLICK_SLOP_PX := 5.0
## The smallest pick box round an animal, pixels.
const PICK_MIN_PX := 10.0

@export var input_enabled := true

var session = null
var mesher: VoxelMesher
var models: VoxelModels
var palette := {}
var species := {}
var dims := Vector3i.ZERO
var voxel_m := 0.125
var generation := -1
var hovered := -1
var highlighted := -1
var following := -1

## Chunk -> [terrain copy 0, terrain copy 1, water copy 0, water copy 1]
var _chunks := {}
## The pan: `x` along the ring (voxels, unwrapped), `y` the camera's height on its own up
## axis (voxels). `_pan` is drawn, `_pan_to` is where it eases to.
var _pan := Vector2.ZERO
var _pan_to := Vector2.ZERO
var _ease_s := PAN_EASE_S
var _dragging := false
var _drag_moved := false
var _press_at := Vector2.ZERO
var _mouse := Vector2(-1, -1)
var _source = null
var _terrain_mat: StandardMaterial3D
var _water_mat: StandardMaterial3D
var _highlight_mat: StandardMaterial3D
var _now := 0.0
var _mesh_ms := 0.0
var _mesh_chunks := 0
var _frame_ms := 0.0
var _shot: SubViewport

@onready var camera: Camera3D = $Camera
@onready var sun: DirectionalLight3D = $Sun
@onready var terrain: Node3D = $Terrain
@onready var organisms: Node3D = $Organisms
@onready var weather: Node3D = $Weather


func _ready() -> void:
	var palette_path := ProjectSettings.globalize_path(PALETTE_PATH)
	palette = JSON.parse_string(FileAccess.get_file_as_string(PALETTE_PATH))
	if typeof(palette) != TYPE_DICTIONARY:
		push_error("WorldView: cannot read " + PALETTE_PATH)
		palette = {}
	mesher = VoxelMesher.new()
	var refused := mesher.load_palette(palette_path)
	if refused != "":
		push_error("WorldView: " + refused)
	_terrain_mat = StandardMaterial3D.new()
	_terrain_mat.vertex_color_use_as_albedo = true
	_terrain_mat.roughness = 1.0
	_terrain_mat.metallic_specular = 0.0
	_water_mat = StandardMaterial3D.new()
	_water_mat.vertex_color_use_as_albedo = true
	_water_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_water_mat.roughness = 1.0
	_water_mat.metallic_specular = 0.0
	_highlight_mat = StandardMaterial3D.new()
	_highlight_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_highlight_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_highlight_mat.albedo_color = Color(1, 1, 1, 0.4)
	_highlight_mat.render_priority = 1
	models = VoxelModels.new()
	species = CubariumSession.new().query("species", null)
	var env := Environment.new()
	camera.environment = env
	weather.setup(sun, env, palette)
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.keep_aspect = Camera3D.KEEP_WIDTH
	camera.near = 0.05
	camera.far = 4000.0
	camera.rotation_degrees = Vector3(-TILT_DEG, 0.0, 0.0)
	camera.current = true
	get_viewport().size_changed.connect(_on_resized)


# --- the API ------------------------------------------------------------------------------

## Draw a live session.
func attach(p_session) -> void:
	if p_session != _source:
		_forget()
	session = p_session
	_source = p_session


## Stop taking from the session. What is drawn stays.
func detach() -> void:
	session = null


## Draw one whole static view (a candidate's `candidate_view(i)`).
func show_view(view: Dictionary) -> void:
	session = null
	_source = null
	_forget()
	_take(view, [], true)
	reset_framing()


func reset_framing() -> void:
	stop_follow()
	var r := _y_range()
	_pan_to = Vector2(0.0, (r.x + r.y) / 2.0)
	_pan = _pan_to
	_place_camera()


func set_highlight(id: int) -> void:
	if highlighted >= 0:
		var old: MeshInstance3D = organisms.animal_node(highlighted)
		if old:
			old.material_overlay = null
	highlighted = id
	var node: MeshInstance3D = organisms.animal_node(id) if id >= 0 else null
	if node:
		node.material_overlay = _highlight_mat


func follow(id: int) -> void:
	following = id


func stop_follow() -> void:
	following = -1


## The current framing rendered at `size`, without anything drawn over the view.
func capture(size: Vector2i) -> Image:
	if _shot == null:
		_shot = SubViewport.new()
		_shot.name = "Capture"
		_shot.world_3d = get_world_3d()
		_shot.render_target_update_mode = SubViewport.UPDATE_DISABLED
		var cam := Camera3D.new()
		cam.projection = Camera3D.PROJECTION_ORTHOGONAL
		cam.keep_aspect = Camera3D.KEEP_WIDTH
		_shot.add_child(cam)
		add_child(_shot)
	var cam: Camera3D = _shot.get_child(0)
	_shot.size = size
	cam.size = camera.size
	cam.near = camera.near
	cam.far = camera.far
	cam.environment = camera.environment
	cam.global_transform = camera.global_transform
	cam.current = true
	# The server's camera now, not at the end of the frame: the draw below is now.
	var rid := cam.get_camera_rid()
	RenderingServer.camera_set_transform(rid, camera.global_transform)
	RenderingServer.camera_set_orthogonal(rid, camera.size, camera.near, camera.far)
	RenderingServer.camera_set_use_vertical_aspect(rid, true)  # KEEP_WIDTH
	_shot.render_target_update_mode = SubViewport.UPDATE_ONCE
	RenderingServer.force_draw(false)
	return _shot.get_texture().get_image()


## One line of what the view costs, for the dev scene.
func perf_line() -> String:
	return "fps %.1f  frame %.2f ms  mesh %.2f ms (%d chunks)  process %.2f ms" % [
		Performance.get_monitor(Performance.TIME_FPS),
		1000.0 / maxf(Performance.get_monitor(Performance.TIME_FPS), 0.001),
		_mesh_ms, _mesh_chunks, _frame_ms]


# --- taking views -------------------------------------------------------------------------

func _process(delta: float) -> void:
	var started := Time.get_ticks_usec()
	_now += delta
	if session != null:
		var v: Dictionary = session.take_view()
		var events: Array = session.take_events()
		_take(v, events, false)
	organisms.advance(_now, delta)
	_input_pan(delta)
	_follow()
	var k := 1.0 - exp(-delta / _ease_s)
	_pan = _pan.lerp(_pan_to, k)
	_place_camera()
	organisms.place(camera.global_position.x)
	weather.advance(delta, camera.global_position.x)
	if _mouse.x >= 0.0 and input_enabled and not _dragging:
		_hover(_mouse)
	_frame_ms = (Time.get_ticks_usec() - started) / 1000.0


func _forget() -> void:
	for nodes in _chunks.values():
		for n in nodes:
			n.queue_free()
	_chunks.clear()
	mesher.reset()
	organisms.clear()
	generation = -1
	hovered = -1


func _take(view: Dictionary, events: Array, instant: bool) -> void:
	if view.is_empty() or not view.has("dims"):
		return
	var d: Vector3i = view["dims"]
	if d.x <= 0:
		return
	var g := int(view.get("generation", 0))
	var renew := g != generation or d != dims or bool(view.get("full", false))
	if renew:
		for nodes in _chunks.values():
			for n in nodes:
				n.queue_free()
		_chunks.clear()
		organisms.clear()
		generation = g
	if d != dims or voxel_m != float(view.get("voxel_m", voxel_m)):
		dims = d
		voxel_m = float(view.get("voxel_m", 0.125))
		_new_world()
	var changed: Array = mesher.apply_view(view)
	_mesh_ms = mesher.last_mesh_ms()
	_mesh_chunks = changed.size()
	for c in changed:
		_set_chunk(c["at"], c["terrain"], c["water"])
	organisms.note_events(events)
	if view.has("tick"):
		if organisms.take_frame(view, _now, instant) and highlighted >= 0:
			set_highlight(highlighted)
		weather.take_sky(view.get("sky", {}), instant)


func _new_world() -> void:
	var dir := ProjectSettings.globalize_path("res://").path_join(MODELS_ROOT).simplify_path()
	dir = dir.path_join(str(roundi(voxel_m * 1000.0)))
	var refused := models.load(dir, ProjectSettings.globalize_path(PALETTE_PATH))
	if refused != "":
		push_warning("WorldView: no baked models (%s); organisms draw as boxes" % refused)
	organisms.setup(models, _terrain_mat, species)
	organisms.set_world(dims, voxel_m)
	weather.set_world(dims)
	terrain.get_node("Copy1").position = Vector3(dims.x, 0, 0)
	reset_framing()


func _set_chunk(at: Vector3i, t: Mesh, w: Mesh) -> void:
	var nodes: Array = _chunks.get(at, [])
	if nodes.is_empty():
		for kind in 2:
			for copy in 2:
				var n := MeshInstance3D.new()
				n.material_override = _terrain_mat if kind == 0 else _water_mat
				n.position = Vector3(at.x * 16, at.y * 16, -at.z * 16)
				terrain.get_node("Copy%d" % copy).add_child(n)
				nodes.append(n)
		_chunks[at] = nodes
	for copy in 2:
		nodes[copy].mesh = t
		nodes[copy].visible = t != null
		nodes[2 + copy].mesh = w
		nodes[2 + copy].visible = w != null


# --- the camera ---------------------------------------------------------------------------

## The world's extent on the camera's up axis, and the view's: `E = H·cos t + D·sin t`.
func _extent() -> float:
	var t := deg_to_rad(TILT_DEG)
	return dims.y * cos(t) + dims.z * sin(t)


func _view_height() -> float:
	var vp := get_viewport().get_visible_rect().size
	return float(dims.x) * vp.y / maxf(vp.x, 1.0)


## The range the camera's centre may take on its up axis: the middle when the world fits.
func _y_range() -> Vector2:
	var e := _extent()
	var v := _view_height()
	if e <= v:
		return Vector2(e / 2.0, e / 2.0)
	return Vector2(v / 2.0, e - v / 2.0)


func _place_camera() -> void:
	if dims.x <= 0:
		return
	var w := float(dims.x)
	var r := _y_range()
	_pan_to.y = clampf(_pan_to.y, r.x, r.y)
	_pan.y = clampf(_pan.y, r.x, r.y)
	camera.size = w
	var t := deg_to_rad(TILT_DEG)
	var up := Vector3(0.0, cos(t), -sin(t))
	var forward := Vector3(0.0, -sin(t), -cos(t))
	# Keep the camera over [W/2, 3W/2): terrain copies at 0 and W cover its span.
	var x := w / 2.0 + fposmod(_pan.x, w)
	camera.global_position = Vector3(x, 0.0, 0.0) + up * _pan.y - forward * (w + dims.y + dims.z)


func _on_resized() -> void:
	_place_camera()


# --- input --------------------------------------------------------------------------------

func _unhandled_input(event: InputEvent) -> void:
	if not input_enabled:
		_dragging = false
		return
	if event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT:
		if event.pressed:
			_dragging = true
			_drag_moved = false
			_press_at = event.position
		else:
			if _dragging and not _drag_moved:
				_hover(event.position)
				animal_clicked.emit(hovered)
			_dragging = false
		get_viewport().set_input_as_handled()
	elif event is InputEventMouseMotion:
		_mouse = event.position
		if _dragging:
			if not _drag_moved and event.position.distance_to(_press_at) > CLICK_SLOP_PX:
				_drag_moved = true
				_drag_by(event.position - _press_at)
			elif _drag_moved:
				_drag_by(event.relative)


func _drag_by(px: Vector2) -> void:
	stop_follow()
	var per_px := float(dims.x) / maxf(get_viewport().get_visible_rect().size.x, 1.0)
	_pan_to.x -= px.x * per_px
	_pan_to.y += px.y * per_px
	_ease_s = DRAG_EASE_S


func _input_pan(delta: float) -> void:
	if not input_enabled or dims.x <= 0:
		return
	var dir := Vector2.ZERO
	if Input.is_key_pressed(KEY_LEFT) or Input.is_key_pressed(KEY_A):
		dir.x -= 1.0
	if Input.is_key_pressed(KEY_RIGHT) or Input.is_key_pressed(KEY_D):
		dir.x += 1.0
	if Input.is_key_pressed(KEY_UP) or Input.is_key_pressed(KEY_W):
		dir.y += 1.0
	if Input.is_key_pressed(KEY_DOWN) or Input.is_key_pressed(KEY_S):
		dir.y -= 1.0
	var stick := Vector2(Input.get_joy_axis(0, JOY_AXIS_LEFT_X), -Input.get_joy_axis(0, JOY_AXIS_LEFT_Y))
	if stick.length() > STICK_DEADZONE:
		dir += stick
	if dir == Vector2.ZERO:
		if not _dragging and following < 0:
			_ease_s = PAN_EASE_S
		return
	stop_follow()
	_pan_to += dir.limit_length(1.0) * PAN_SPEED * float(dims.x) * delta
	_ease_s = PAN_EASE_S


## Pan by a scripted amount (view widths along the ring, view heights up), eased: the dev
## scene's pan demonstration.
func pan_by(views: Vector2) -> void:
	stop_follow()
	_pan_to += Vector2(views.x * dims.x, views.y * _view_height())
	_ease_s = FOLLOW_EASE_S


func _follow() -> void:
	if following < 0:
		return
	var at = organisms.animal_at(following)
	if at == null:
		stop_follow()
		return
	var w := float(dims.x)
	var cam_x := w / 2.0 + fposmod(_pan.x, w)
	_pan_to.x = _pan.x + wrapf(at.x - cam_x, -w / 2.0, w / 2.0)
	var t := deg_to_rad(TILT_DEG)
	_pan_to.y = at.y * cos(t) + at.z * sin(t)
	_ease_s = FOLLOW_EASE_S


# --- picking ------------------------------------------------------------------------------

## The animal under screen point `p`: the nearest to the camera whose projected model box
## holds it.
func _pick(p: Vector2) -> int:
	var best := -1
	var best_depth := INF
	var forward := -camera.global_transform.basis.z
	for entry in organisms.live_animals():
		var node: MeshInstance3D = entry[1]
		if node.mesh == null:
			continue
		var box := node.global_transform * node.mesh.get_aabb()
		var lo := Vector2(INF, INF)
		var hi := Vector2(-INF, -INF)
		for i in 8:
			var s := camera.unproject_position(box.get_endpoint(i))
			lo = lo.min(s)
			hi = hi.max(s)
		var grow := (Vector2(PICK_MIN_PX, PICK_MIN_PX) - (hi - lo)).max(Vector2.ZERO) / 2.0
		if Rect2(lo - grow, hi - lo + 2.0 * grow).has_point(p):
			var depth := forward.dot(box.get_center() - camera.global_position)
			if depth < best_depth:
				best_depth = depth
				best = entry[0]
	return best


func _hover(p: Vector2) -> void:
	var id := _pick(p)
	if id != hovered:
		hovered = id
		animal_hovered.emit(id)
