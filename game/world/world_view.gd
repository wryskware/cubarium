class_name WorldView
extends Control
## **The world, drawn by the Rust renderer** (H1 of design/handoffs/hybrid-renderer-2026-10-07.md,
## D-0005): `cubarium-gpu`'s voxel walk, its lighting and sky, at the pixel-scale raster,
## shown full-size under the UI with the desktop window's fit and nearest upscale. The
## renderer runs on the session's loop and a presenter thread of its own
## (`WorldRenderer`); this node takes its newest frame each `_process`.
##
## API (the shell's contract, design/handoffs/godot-direction-2026-10-07.md):
## - `attach(session)` draws a live `CubariumSession` (the renderer hooks into its loop);
##   `detach()` stops (the last frame stays).
## - `show_view(view)` draws a candidate: `view` is `candidate_ref(i)` (or `candidate_view(i)`),
##   which names its session and index (`source`, `candidate`); the candidate's own world is
##   rendered, once. With input off (a thumbnail) it is staged in its turn, one thumbnail at
##   a time across the app, and its renderer is freed after the first frame.
## - `input_enabled`: false ignores pointer and pan input (a menu is open).
## - `reset_framing()`: the whole strip in view, the ring unturned.
## - Signals `animal_hovered(id)` (−1 for none) and `animal_clicked(id)` (−1 for empty
##   ground). A press and release under 5 px of movement is a click; more is a drag.
## - `set_highlight(id)`: the renderer draws that animal's cells leaning toward white (−1
##   clears). `follow(id)` keeps it in view, easing; `stop_follow()`. Panning by hand stops
##   following.
## - `set_animal_motion(movement, subpixel, startup := false)`: how the renderer draws
##   animals, live: `movement` "smooth" or "stepped", `subpixel` (smooth only). `startup`
##   marks a viewer's stored settings, under what the environment named at startup
##   (`CUBARIUM_ANIMALS`, `CUBARIUM_ANIMAL_SUBPIXEL`); a change while it runs applies whole.
##   Kept and applied when the renderer starts if it has not yet. `animal_motion()` is the
##   renderer's view of them (`WorldRenderer.animal_motion`), {} before it starts.
## - `capture(size) -> Image`: the latest frame as framed, fitted into `size`.
## - `problem() -> String`: why nothing is drawn yet, when the renderer did not start or
##   gave up ("" otherwise), so a thumbnail can say so instead of staying blank.
##
## Mouse wheel zooms toward the pointer (1–4×); Home restores the overview. Drag,
## arrows/WASD and the left stick pan in both axes. The camera lives in baseline raster
## coordinates: Rust renders integer detail tiers and cropped frames, while Godot smooths
## the fractional camera movement and zoom without changing the world projection.

signal animal_hovered(id: int)
signal animal_clicked(id: int)

## The presentation config (lighting tier, `[light]`, haze, `px_per_voxel`), relative to the
## repository root.
const DEFAULT_CONFIG := "config/desktop/terrarium.toml"
const FPS := 60.0
const SCOPE := preload("res://ui/neon/zoom_scope.gdshader")
const ZOOM_MIN := 1.0
const ZOOM_MAX := 4.0
const ZOOM_WHEEL_STEP := 1.16
const ZOOM_EASE_S := 0.10
const PAN_EASE_S := 0.12
const DRAG_EASE_S := 0.05
const FOLLOW_EASE_S := 0.35
## Key and stick pan speed: view widths per second.
const PAN_SPEED := 0.5
const STICK_DEADZONE := 0.2
## A press that moves less than this many pixels before release is a click.
const CLICK_SLOP_PX := 5.0
## The smallest pick box round an animal, screen pixels.
const PICK_MIN_PX := 10.0

## Area-average in linear light for fractional fits; nearest at whole pixel scales.
## All samples use the same baseline camera transform as picking, follow and captures.
const SHADER := """
shader_type canvas_item;
render_mode blend_disabled;
uniform vec2 view_size = vec2(1.0);
uniform vec2 raster_size = vec2(1.0);
uniform vec2 world_size = vec2(1.0);
uniform vec2 camera_center = vec2(0.5);
uniform vec2 source_center = vec2(0.5);
uniform float step_px = 1.0;
uniform bool wrap_frame = true;
const int MAX_TAPS = 8;
vec3 dec(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c)); }
vec3 enc(vec3 c) {
	c = max(c, vec3(0.0));
	return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
void fragment() {
	vec2 panel = floor(UV * view_size);
	vec2 delta = (panel + 0.5 - view_size * 0.5) * step_px;
	vec2 world = camera_center + delta;
	vec2 centre = source_center + delta;
	vec2 lo = centre - 0.5 * step_px;
	vec2 hi = centre + 0.5 * step_px;
	if (abs(delta.x) - 0.5 * step_px >= world_size.x * 0.5 || world.y + 0.5 * step_px <= 0.0 || world.y - 0.5 * step_px >= world_size.y
		|| hi.y <= 0.0 || lo.y >= raster_size.y || (!wrap_frame && (hi.x <= 0.0 || lo.x >= raster_size.x))) {
		COLOR = vec4(0.0, 0.0, 0.0, 1.0);
	} else {
		vec2 over = max(hi - lo - float(MAX_TAPS - 1), vec2(0.0));
		lo += 0.5 * over;
		hi -= 0.5 * over;
		lo.y = max(lo.y, 0.0);
		hi.y = min(hi.y, raster_size.y);
		if (!wrap_frame) { lo.x = max(lo.x, 0.0); hi.x = min(hi.x, raster_size.x); }
		int rw = int(wrap_frame ? world_size.x : raster_size.x);
		int x0 = int(floor(lo.x));
		int x1 = int(ceil(hi.x)) - 1;
		int y0 = int(floor(lo.y));
		int y1 = min(int(ceil(hi.y)) - 1, int(raster_size.y) - 1);
		vec3 sum = vec3(0.0);
		float total = 0.0;
		for (int y = y0; y <= y1 && y < y0 + MAX_TAPS; y++) {
			float wy = min(hi.y, float(y + 1)) - max(lo.y, float(y));
			if (wy <= 1e-4) { continue; }
			for (int x = x0; x <= x1 && x < x0 + MAX_TAPS; x++) {
				float wx = min(hi.x, float(x + 1)) - max(lo.x, float(x));
				if (wx <= 1e-4) { continue; }
				int xr = ((x % rw) + rw) % rw;
				sum += wx * wy * dec(texelFetch(TEXTURE, ivec2(xr, y), 0).rgb);
				total += wx * wy;
			}
		}
		COLOR = total <= 1e-4 ? vec4(0.0, 0.0, 0.0, 1.0) : vec4(enc(sum / total), 1.0);
	}
}
"""

## When false, pointer and pan input are ignored (the shell turns it off under a menu).
var input_enabled := true:
	set(v):
		input_enabled = v
		if not v:
			_dragging = false
			_set_hovered(-1)

## A presentation config to use instead of the desktop terrarium's (an absolute path, or
## relative to the repository root). Set before `attach` or `show_view`.
var config_path := ""

var session = null
var renderer = null
## The animals' live settings asked for, [movement, subpixel, startup]; [] for the config's.
var motion := []
var highlighted := -1
var following := -1
var hovered := -1
## The latest frame: its raster, its turn round the ring, its world, its tick.
var raster_size := Vector2i.ZERO
## Full overview dimensions, unchanged by detail tiers and camera crops.
var baseline_size := Vector2i.ZERO
var frame_view_x := 0
var frame_view_y := 0
var frame_factor := 1.0
var frame_s := 1
var frame_base_s := 1
var scope_enabled := true
var frame_generation := -1
var frame_tick := -1
var frames_shown := 0
## Milliseconds spent turning frames into the texture (create + upload), cumulative.
var upload_ms := 0.0

## Camera centre in baseline raster pixels; x remains unwrapped round the ring.
var _center := Vector2.ZERO
var _center_to := Vector2.ZERO
var _zoom := 1.0
var _zoom_to := 1.0
var _zoom_anchor := Vector2.ZERO
var _zoom_anchor_world := Vector2.ZERO
var _zoom_anchor_active := false
var _scope: ColorRect
var _scope_material: ShaderMaterial
var _ease_s := PAN_EASE_S
var _dragging := false
var _drag_moved := false
var _press_at := Vector2.ZERO
var _mouse := Vector2(-1, -1)
var _image: Image
var _texture: ImageTexture
var _material: ShaderMaterial
## What to draw once the view has a size: ["live", session] or ["candidate", session, i].
var _pending: Array = []
var _static := false
var _process_ms := 0.0
## Why the renderer did not start, or "".
var _refused := ""


func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_STOP
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_material = ShaderMaterial.new()
	var shader := Shader.new()
	shader.code = SHADER
	_material.shader = shader
	material = _material
	_scope = ColorRect.new()
	_scope.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_scope.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_scope_material = ShaderMaterial.new()
	_scope_material.shader = SCOPE
	_scope.material = _scope_material
	add_child(_scope)


func _exit_tree() -> void:
	if renderer != null:
		renderer.detach()


# --- the contract --------------------------------------------------------------------------

func attach(p_session) -> void:
	session = p_session
	_static = false
	_pending = ["live", p_session]


func detach() -> void:
	if renderer != null:
		renderer.detach()
	session = null
	_pending = []


func show_view(view: Dictionary) -> void:
	detach()
	_static = true
	if view.has("source") and view.has("candidate"):
		_pending = ["candidate", view["source"], int(view["candidate"])]
	else:
		push_warning("WorldView: a view that names no candidate (source, candidate) cannot be rendered")


func reset_framing() -> void:
	stop_follow()
	_zoom_anchor_active = false
	_zoom = ZOOM_MIN
	_zoom_to = ZOOM_MIN
	_center = Vector2(baseline_size) * 0.5
	_center_to = _center
	_apply_pan()


func set_scope_enabled(enabled: bool) -> void:
	scope_enabled = enabled
	if _scope_material != null:
		_scope_material.set_shader_parameter("enabled", enabled)


func set_highlight(id: int) -> void:
	highlighted = id
	if renderer != null:
		renderer.set_highlight(id)


func set_animal_motion(movement: String, subpixel: bool, startup := false) -> void:
	# A startup setting never undoes a viewer's change made before the renderer started.
	if startup and not motion.is_empty() and not motion[2]:
		return
	motion = [movement, subpixel, startup]
	if renderer != null:
		_apply_motion()


func animal_motion() -> Dictionary:
	return renderer.animal_motion() if renderer != null else {}


func _apply_motion() -> void:
	if motion.is_empty():
		return
	var refused: String = renderer.set_animal_motion(motion[0], motion[1], motion[2])
	if refused != "":
		push_warning("WorldView: animal settings refused (%s)" % refused)


func follow(id: int) -> void:
	following = id


func stop_follow() -> void:
	following = -1


## The latest frame as framed now (its turn round the ring included), fitted into `size`
## with black bars, without anything drawn over the view.
func capture(target_size: Vector2i) -> Image:
	var out := Image.create_empty(maxi(target_size.x, 1), maxi(target_size.y, 1), false, Image.FORMAT_RGBA8)
	out.fill(Color.BLACK)
	if _image == null or size.x <= 0.0 or size.y <= 0.0:
		return out
	var scale := minf(float(target_size.x) / size.x, float(target_size.y) / size.y)
	var fit := Vector2i(maxi(1, int(size.x * scale)), maxi(1, int(size.y * scale)))
	var origin := (target_size - fit) / 2
	for y in fit.y:
		for x in fit.x:
			var p := Vector2((x + 0.5) * size.x / fit.x, (y + 0.5) * size.y / fit.y)
			var source := _screen_to_frame(p)
			if source.x >= 0.0:
				out.set_pixel(origin.x + x, origin.y + y, _image.get_pixel(int(source.x), int(source.y)))
	return out


## Why nothing is drawn: the renderer refused to start, or reported an error before its
## first frame. "" while it is on its way or once a frame is shown.
func problem() -> String:
	if _refused != "":
		return _refused
	if renderer != null and frames_shown == 0:
		return renderer.error()
	return ""


## One line of what the view costs, for the dev scene.
func perf_line() -> String:
	var s: Dictionary = renderer.stats() if renderer != null else {}
	var n := maxf(float(s.get("frames", 0)), 1.0)
	return "fps %.1f  godot frame %.2f ms (process %.2f)  frames %d  GPU %.2f ms  record %.2f ms  copy %.2f ms  upload %.2f ms  pack %.2f ms/tick (pack thread)  copy %.2f ms/tick (loop)  view %.2f ms" % [
		Performance.get_monitor(Performance.TIME_FPS),
		1000.0 / maxf(Performance.get_monitor(Performance.TIME_FPS), 0.001),
		Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0, int(s.get("frames", 0)),
		float(s.get("gpu_ms", 0.0)) / n, float(s.get("record_ms", 0.0)) / n,
		float(s.get("copy_ms", 0.0)) / n, upload_ms / maxf(frames_shown, 1.0),
		float(s.get("pack_ms", 0.0)) / maxf(float(s.get("packs", 0)), 1.0),
		float(s.get("snapshot_ms", 0.0)) / maxf(float(s.get("snapshots", 0)), 1.0), _process_ms]


# --- frames ----------------------------------------------------------------------------------

func _process(delta: float) -> void:
	var started := Time.get_ticks_usec()
	if not _pending.is_empty() and size.x >= 1.0 and size.y >= 1.0:
		_begin()
	if renderer != null:
		_take_frame()
	_input_pan(delta)
	_follow()
	_zoom = lerpf(_zoom, _zoom_to, 1.0 - exp(-delta / ZOOM_EASE_S))
	if absf(_zoom - _zoom_to) < 0.0001:
		_zoom = _zoom_to
	if _zoom_anchor_active:
		_center = _zoom_anchor_world - _zoom_anchor * _step()
		_center_to = _zoom_anchor_world - _zoom_anchor * _base_step() / _zoom_to
		_zoom_anchor_active = _zoom != _zoom_to
	else:
		_center = _center.lerp(_center_to, 1.0 - exp(-delta / _ease_s))
	_apply_pan()
	if _mouse.x >= 0.0 and input_enabled and not _dragging:
		_hover(_mouse)
	_process_ms = (Time.get_ticks_usec() - started) / 1000.0


## Start the renderer (once, at this view's size) and hand it what to draw.
func _begin() -> void:
	var what := _pending
	_pending = []
	if renderer == null:
		renderer = WorldRenderer.new()
		var root := ProjectSettings.globalize_path("res://").path_join("..").simplify_path()
		var cfg := config_path if config_path != "" else DEFAULT_CONFIG
		if cfg.is_relative_path():
			cfg = root.path_join(cfg)
		var refused: String = renderer.start(cfg, root, Vector2i(size), FPS)
		if refused != "":
			push_error("WorldView: no renderer (%s)" % refused)
			_refused = refused
			renderer = null
			return
		if highlighted >= 0:
			renderer.set_highlight(highlighted)
		_apply_motion()
	match what[0]:
		"live":
			renderer.attach(what[1])
		"candidate":
			if input_enabled:
				renderer.show_candidate(what[1], what[2])
			else:
				renderer.show_thumbnail(what[1], what[2])


func _take_frame() -> void:
	var f = renderer.take_frame()
	if f == null:
		return
	var at := Time.get_ticks_usec()
	var w := int(f["width"])
	var h := int(f["height"])
	var img := Image.create_from_data(w, h, false, Image.FORMAT_RGBA8, f["rgba"])
	if _texture == null or Vector2i(w, h) != raster_size:
		_texture = ImageTexture.create_from_image(img)
	else:
		_texture.update(img)
	upload_ms += (Time.get_ticks_usec() - at) / 1000.0
	_image = img
	var fresh := int(f["generation"]) != frame_generation or baseline_size == Vector2i.ZERO
	raster_size = Vector2i(w, h)
	baseline_size = Vector2i(int(f.get("ring_width", w)), int(f.get("full_height", h)))
	frame_s = int(f.get("s", 1))
	frame_base_s = int(f.get("base_s", 1))
	frame_factor = float(frame_s) / maxf(float(frame_base_s), 1.0)
	frame_view_x = int(f["view_x"])
	frame_view_y = int(f.get("view_y", 0))
	frame_generation = int(f["generation"])
	frame_tick = int(f["tick"])
	frames_shown += 1
	if fresh:
		reset_framing()
	_material.set_shader_parameter("raster_size", Vector2(raster_size))
	queue_redraw()
	# A still thumbnail is drawn once; its renderer's GPU memory goes.
	if _static and not input_enabled:
		renderer.release()


func _draw() -> void:
	if _texture != null:
		draw_texture_rect(_texture, Rect2(Vector2.ZERO, size), false)
	else:
		draw_rect(Rect2(Vector2.ZERO, size), Color.BLACK)


# --- framing -------------------------------------------------------------------------------

## Baseline raster pixels per screen pixel at the overview and at the drawn zoom.
func _base_step() -> float:
	if baseline_size.x <= 0 or size.x <= 0.0 or size.y <= 0.0:
		return 1.0
	return maxf(baseline_size.x / size.x, baseline_size.y / size.y)


func _step() -> float:
	return _base_step() / _zoom


## Bounds on the camera centre: when the full strip fits vertically it stays centred.
func _y_range(zoom := -1.0) -> Vector2:
	var z := _zoom if zoom < 0.0 else zoom
	var half_visible := size.y * _base_step() / z * 0.5
	var height := float(baseline_size.y)
	return Vector2(half_visible, height - half_visible) if half_visible * 2.0 < height else Vector2.ONE * height * 0.5


func _wrapped_delta(x: float, width: float) -> float:
	return fposmod(x + width * 0.5, width) - width * 0.5


## The current camera centre inside the latest cropped detail frame, the short way round.
func _source_center() -> Vector2:
	var rw := float(maxi(baseline_size.x, 1)) * frame_factor
	var frame_center := float(frame_view_x) + raster_size.x * 0.5
	return Vector2(raster_size.x * 0.5 + _wrapped_delta(_center.x * frame_factor - frame_center, rw),
		_center.y * frame_factor - frame_view_y)


func _wrap_frame() -> bool:
	return raster_size.x >= baseline_size.x * frame_factor


func _apply_pan() -> void:
	var r := _y_range()
	var target := _y_range(_zoom_to)
	_center_to.y = clampf(_center_to.y, target.x, target.y)
	_center.y = clampf(_center.y, r.x, r.y)
	if renderer != null and baseline_size.x > 0:
		renderer.set_camera(_zoom, _center, Vector2i(size))
	if _material != null:
		_material.set_shader_parameter("view_size", size)
		_material.set_shader_parameter("raster_size", Vector2(raster_size))
		_material.set_shader_parameter("world_size", Vector2(baseline_size) * frame_factor)
		_material.set_shader_parameter("camera_center", _center * frame_factor)
		_material.set_shader_parameter("source_center", _source_center())
		_material.set_shader_parameter("step_px", _step() * frame_factor)
		_material.set_shader_parameter("wrap_frame", _wrap_frame())
	if _scope_material != null:
		_scope_material.set_shader_parameter("view_size", size)
		_scope_material.set_shader_parameter("zoom", _zoom)
		_scope_material.set_shader_parameter("enabled", scope_enabled)
	queue_redraw()


## The baseline projected world under a screen point; off the picture returns (-1, -1).
func _baseline_at(p: Vector2) -> Vector2:
	if baseline_size.x <= 0 or not Rect2(Vector2.ZERO, size).has_point(p):
		return Vector2(-1, -1)
	var delta := (p - size * 0.5) * _step()
	var world := _center + delta
	if absf(delta.x) >= baseline_size.x * 0.5 or world.y < 0.0 or world.y >= baseline_size.y:
		return Vector2(-1, -1)
	world.x = fposmod(world.x, float(baseline_size.x))
	return world


## Shared source transform for snapshots and the presentation shader.
func _screen_to_frame(p: Vector2) -> Vector2:
	if _baseline_at(p).x < 0.0:
		return Vector2(-1, -1)
	var source := _source_center() + (p - size * 0.5) * _step() * frame_factor
	if _wrap_frame():
		source.x = fposmod(source.x, float(maxi(baseline_size.x, 1)) * frame_factor)
	if source.x < 0.0 or source.x >= raster_size.x or source.y < 0.0 or source.y >= raster_size.y:
		return Vector2(-1, -1)
	return source


## Baseline world raster pixel for picking; (-1, -1) outside the shown picture.
func screen_to_raster(p: Vector2) -> Vector2i:
	var at := _baseline_at(p)
	return Vector2i(-1, -1) if at.x < 0.0 else Vector2i(int(floor(at.x)), int(floor(at.y)))


## A baseline world raster pixel's centre as a screen point, the inverse of picking.
func raster_to_screen(r: Vector2) -> Vector2:
	var delta := Vector2(_wrapped_delta(r.x + 0.5 - _center.x, float(maxi(baseline_size.x, 1))), r.y + 0.5 - _center.y)
	return size * 0.5 + delta / _step()


# --- input ---------------------------------------------------------------------------------

func _gui_input(event: InputEvent) -> void:
	if not _input_allowed():
		_dragging = false
		return
	if event is InputEventMouseButton and event.pressed and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		var direction := 1.0 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1.0
		zoom_at(_zoom_to * pow(ZOOM_WHEEL_STEP, direction * maxf(event.factor, 1.0)), event.position)
		accept_event()
	elif event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT:
		if event.pressed:
			_dragging = true
			_drag_moved = false
			_press_at = event.position
		else:
			if _dragging and not _drag_moved:
				_hover(event.position)
				animal_clicked.emit(hovered)
			_dragging = false
		accept_event()
	elif event is InputEventMouseMotion:
		_mouse = event.position
		if _dragging:
			if not _drag_moved and event.position.distance_to(_press_at) > CLICK_SLOP_PX:
				_drag_moved = true
				_drag_by(event.position - _press_at)
			elif _drag_moved:
				_drag_by(event.relative)


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT:
		_mouse = Vector2(-1, -1)
		if not _dragging:
			_set_hovered(-1)
	elif what == NOTIFICATION_RESIZED:
		_apply_pan()


func _drag_by(px: Vector2) -> void:
	stop_follow()
	_zoom_anchor_active = false
	_center_to -= px * _step()
	_ease_s = DRAG_EASE_S


func _input_pan(delta: float) -> void:
	if not _input_allowed() or baseline_size.x <= 0:
		return
	var dir := Vector2.ZERO
	if Input.is_key_pressed(KEY_LEFT) or Input.is_key_pressed(KEY_A):
		dir.x -= 1.0
	if Input.is_key_pressed(KEY_RIGHT) or Input.is_key_pressed(KEY_D):
		dir.x += 1.0
	if Input.is_key_pressed(KEY_UP) or Input.is_key_pressed(KEY_W):
		dir.y -= 1.0
	if Input.is_key_pressed(KEY_DOWN) or Input.is_key_pressed(KEY_S):
		dir.y += 1.0
	var stick := Vector2(Input.get_joy_axis(0, JOY_AXIS_LEFT_X), Input.get_joy_axis(0, JOY_AXIS_LEFT_Y))
	if stick.length() > STICK_DEADZONE:
		dir += stick
	if dir == Vector2.ZERO:
		if not _dragging and following < 0:
			_ease_s = PAN_EASE_S
		return
	stop_follow()
	_zoom_anchor_active = false
	_center_to += dir.limit_length(1.0) * PAN_SPEED * size.x * _step() * delta
	_ease_s = PAN_EASE_S


## Pan by a scripted amount (view widths round the ring, view heights down), eased: the dev
## scene's pan.
func pan_by(views: Vector2) -> void:
	stop_follow()
	_zoom_anchor_active = false
	_center_to += Vector2(views.x * size.x, views.y * size.y) * _step()
	_ease_s = FOLLOW_EASE_S


func _follow() -> void:
	if following < 0 or renderer == null or baseline_size.x <= 0:
		return
	var at: Vector2 = renderer.animal_at(following)
	if at.x < 0.0:
		stop_follow()
		return
	_zoom_anchor_active = false
	_center_to.x = _center.x + _wrapped_delta(at.x - _center.x, float(baseline_size.x))
	_center_to.y = at.y
	_ease_s = FOLLOW_EASE_S


func _input_allowed() -> bool:
	if not input_enabled or not is_visible_in_tree():
		return false
	var focus := get_viewport().gui_get_focus_owner()
	return not (focus is LineEdit or focus is TextEdit)


func _unhandled_key_input(event: InputEvent) -> void:
	if _input_allowed() and event is InputEventKey and event.pressed and not event.echo and event.physical_keycode == KEY_HOME:
		reset_framing()
		get_viewport().set_input_as_handled()


## Set an eased zoom with the world point under the cursor held still, except at y bounds.
func zoom_at(value: float, p: Vector2) -> void:
	if baseline_size.x <= 0:
		return
	var next := clampf(value, ZOOM_MIN, ZOOM_MAX)
	if is_equal_approx(next, _zoom_to):
		return
	stop_follow()
	_zoom_anchor = p - size * 0.5
	_zoom_anchor_world = _center + _zoom_anchor * _step()
	_zoom_to = next
	_center_to = _zoom_anchor_world - _zoom_anchor * _base_step() / _zoom_to
	_zoom_anchor_active = true


# --- picking -------------------------------------------------------------------------------

## The animal under screen point `p`: the renderer's projection inverted, or the nearest
## drawn body within half the smallest pick box.
func pick(p: Vector2) -> int:
	if renderer == null:
		return -1
	var r := screen_to_raster(p)
	if r.x < 0:
		return -1
	return renderer.pick(r.x, r.y, PICK_MIN_PX * 0.5 * _step())


## Where animal `id` is on screen, or (-1, -1) when it is not drawn.
func animal_screen(id: int) -> Vector2:
	if renderer == null:
		return Vector2(-1, -1)
	var at: Vector2 = renderer.animal_at(id)
	return Vector2(-1, -1) if at.x < 0.0 else raster_to_screen(at)


func _hover(p: Vector2) -> void:
	_set_hovered(pick(p))


func _set_hovered(id: int) -> void:
	if id != hovered:
		hovered = id
		animal_hovered.emit(id)
