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
## - `capture(size) -> Image`: the latest frame as framed, fitted into `size`.
## - `problem() -> String`: why nothing is drawn yet, when the renderer did not start or
##   gave up ("" otherwise), so a thumbnail can say so instead of staying blank.
##
## Panning: mouse drag, arrows/WASD, the gamepad's left stick, eased. `x` turns round the
## ring without end: the renderer turns its raster by whole pixels and the frame is shifted
## here by the fraction left over, so the camera glides while the world's pixels stay locked
## to each other. `y` moves only when the strip is taller than the view.

signal animal_hovered(id: int)
signal animal_clicked(id: int)

## The presentation config (lighting tier, `[light]`, haze, `px_per_voxel`), relative to the
## repository root.
const DEFAULT_CONFIG := "config/desktop/terrarium.toml"
const FPS := 60.0
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

## The desktop window's present pass (`present.frag`, `WindowFit`), with the ring's turn:
## each screen pixel the area average of the raster it covers, in linear light (exactly the
## nearest upscale at a whole scale), the picture centred with black bars, and `x` wrapping
## round the ring by `shift`.
const SHADER := """
shader_type canvas_item;
render_mode blend_disabled;
uniform vec2 view_size = vec2(1.0);
uniform vec2 raster_size = vec2(1.0);
uniform vec2 shift = vec2(0.0);
const int MAX_TAPS = 8;
vec3 dec(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c)); }
vec3 enc(vec3 c) {
	c = max(c, vec3(0.0));
	return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
void fragment() {
	vec2 panel = floor(UV * view_size);
	float st = max(raster_size.x / view_size.x, raster_size.y / view_size.y);
	vec2 corner = 0.5 * (view_size - raster_size / st);
	vec2 centre = st * (panel + 0.5 - corner);
	vec2 lo = centre - 0.5 * st;
	vec2 hi = centre + 0.5 * st;
	if (any(lessThanEqual(hi, vec2(0.0))) || any(greaterThanEqual(lo, raster_size))) {
		COLOR = vec4(0.0, 0.0, 0.0, 1.0);
	} else {
		// The picture's own edges stay where the fit puts them; inside, x turns round the ring.
		lo.x = max(lo.x, 0.0);
		hi.x = min(hi.x, raster_size.x);
		lo += shift;
		hi += shift;
		vec2 span = hi - lo;
		vec2 over = max(span - float(MAX_TAPS - 1), vec2(0.0));
		lo += 0.5 * over;
		hi -= 0.5 * over;
		lo.y = max(lo.y, 0.0);
		hi.y = min(hi.y, raster_size.y);
		int rw = int(raster_size.x);
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
var highlighted := -1
var following := -1
var hovered := -1
## The latest frame: its raster, its turn round the ring, its world, its tick.
var raster_size := Vector2i.ZERO
var frame_view_x := 0
var frame_generation := -1
var frame_tick := -1
var frames_shown := 0
## Milliseconds spent turning frames into the texture (create + upload), cumulative.
var upload_ms := 0.0

## The pan: `x` raster columns round the ring (unwrapped), `y` raster rows. `_pan` is
## drawn, `_pan_to` is where it eases to.
var _pan := Vector2.ZERO
var _pan_to := Vector2.ZERO
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
	_pan = Vector2(0.0, _y_range().x)
	_pan_to = _pan
	_apply_pan()


func set_highlight(id: int) -> void:
	highlighted = id
	if renderer != null:
		renderer.set_highlight(id)


func follow(id: int) -> void:
	following = id


func stop_follow() -> void:
	following = -1


## The latest frame as framed now (its turn round the ring included), fitted into `size`
## with black bars, without anything drawn over the view.
func capture(size: Vector2i) -> Image:
	var out := Image.create_empty(maxi(size.x, 1), maxi(size.y, 1), false, Image.FORMAT_RGBA8)
	out.fill(Color.BLACK)
	if _image == null:
		return out
	var w := _image.get_width()
	var h := _image.get_height()
	var turned := Image.create_empty(w, h, false, Image.FORMAT_RGBA8)
	var s := posmod(int(round(_shift().x)), w)
	turned.blit_rect(_image, Rect2i(s, 0, w - s, h), Vector2i(0, 0))
	if s > 0:
		turned.blit_rect(_image, Rect2i(0, 0, s, h), Vector2i(w - s, 0))
	var k := minf(float(size.x) / w, float(size.y) / h)
	var fit := Vector2i(maxi(1, int(w * k)), maxi(1, int(h * k)))
	turned.resize(fit.x, fit.y, Image.INTERPOLATE_LANCZOS)
	out.blit_rect(turned, Rect2i(Vector2i.ZERO, fit), (size - fit) / 2)
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
	var k := 1.0 - exp(-delta / _ease_s)
	_pan = _pan.lerp(_pan_to, k)
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
	var fresh := int(f["generation"]) != frame_generation or Vector2i(w, h) != raster_size
	raster_size = Vector2i(w, h)
	frame_view_x = int(f["view_x"])
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

## Raster pixels per screen pixel (the fit).
func _step() -> float:
	if raster_size.x <= 0 or size.x <= 0.0 or size.y <= 0.0:
		return 1.0
	return maxf(raster_size.x / size.x, raster_size.y / size.y)


## The range `_pan.y` may take: 0 when the strip fits the view's height.
func _y_range() -> Vector2:
	var visible := size.y * _step()
	var extra := float(raster_size.y) - visible
	return Vector2(0.0, maxf(extra, 0.0)) if extra > 0.0 else Vector2.ZERO


## The shift the shown frame needs: the pan, less the whole columns the frame is already
## turned, the short way round.
func _shift() -> Vector2:
	var rw := float(maxi(raster_size.x, 1))
	var sx := fposmod(_pan.x - frame_view_x + rw / 2.0, rw) - rw / 2.0
	var r := _y_range()
	var sy := _pan.y - (r.x + r.y) / 2.0 if r.y > 0.0 else 0.0
	return Vector2(sx, sy)


func _apply_pan() -> void:
	var r := _y_range()
	_pan_to.y = clampf(_pan_to.y, r.x, r.y)
	_pan.y = clampf(_pan.y, r.x, r.y)
	if renderer != null and raster_size.x > 0:
		renderer.set_view_x(posmod(int(floor(_pan.x)), raster_size.x))
	if _material != null:
		_material.set_shader_parameter("view_size", size)
		_material.set_shader_parameter("shift", _shift())
	queue_redraw()


## Screen point `p` (this node's pixels) as a world raster pixel, or (-1, -1) off the
## picture.
func screen_to_raster(p: Vector2) -> Vector2i:
	if raster_size.x <= 0:
		return Vector2i(-1, -1)
	var st := _step()
	var corner := 0.5 * (size - Vector2(raster_size) / st)
	var c := st * (p - corner)
	if c.x < 0.0 or c.y < 0.0 or c.x >= raster_size.x or c.y >= raster_size.y:
		return Vector2i(-1, -1)
	c += _shift()
	return Vector2i(posmod(int(floor(c.x)) + frame_view_x, raster_size.x), int(floor(c.y)))


## A world raster pixel as a screen point (this node's pixels), the inverse of
## `screen_to_raster` at the pixel's centre.
func raster_to_screen(r: Vector2) -> Vector2:
	var st := _step()
	var corner := 0.5 * (size - Vector2(raster_size) / st)
	var rw := float(maxi(raster_size.x, 1))
	var c := Vector2(fposmod(r.x + 0.5 - frame_view_x, rw), r.y + 0.5) - _shift()
	c.x = fposmod(c.x, rw)
	return c / st + corner


# --- input ---------------------------------------------------------------------------------

func _gui_input(event: InputEvent) -> void:
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
	_pan_to -= px * _step()
	_ease_s = DRAG_EASE_S


func _input_pan(delta: float) -> void:
	if not input_enabled or raster_size.x <= 0 or not is_visible_in_tree():
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
	_pan_to += dir.limit_length(1.0) * PAN_SPEED * float(raster_size.x) * delta
	_ease_s = PAN_EASE_S


## Pan by a scripted amount (view widths round the ring, view heights down), eased: the dev
## scene's pan.
func pan_by(views: Vector2) -> void:
	stop_follow()
	_pan_to += Vector2(views.x * raster_size.x, views.y * size.y * _step())
	_ease_s = FOLLOW_EASE_S


func _follow() -> void:
	if following < 0 or renderer == null or raster_size.x <= 0:
		return
	var at: Vector2 = renderer.animal_at(following)
	if at.x < 0.0:
		stop_follow()
		return
	var rw := float(raster_size.x)
	_pan_to.x = _pan.x + wrapf(at.x - (rw / 2.0 + _pan.x), -rw / 2.0, rw / 2.0)
	var r := _y_range()
	_pan_to.y = at.y - size.y * _step() / 2.0 if r.y > 0.0 else 0.0
	_ease_s = FOLLOW_EASE_S


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
