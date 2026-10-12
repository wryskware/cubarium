extends SceneTree
## Short mechanism tests: layout fit, shared camera transform, cursor anchor and input gates.
## Run headless through labd: godot --headless --path game --script res://tests/view_fit_test.gd

const Fit := preload("res://ui/window_fit.gd")
const View := preload("res://world/world_view.gd")
var failures := 0


class RendererStub:
	extends RefCounted
	var camera: Array = []
	var picked := Vector2i(-1, -1)
	var animal := Vector2(230, 76)
	func set_camera(zoom: float, center: Vector2, screen: Vector2i) -> void:
		camera = [zoom, center, screen]
	func take_frame():
		return null
	func pick(x: int, y: int, _radius: float) -> int:
		picked = Vector2i(x, y)
		return 7
	func animal_at(_id: int) -> Vector2:
		return animal
	func detach() -> void:
		pass


func check(ok: bool, message: String) -> void:
	print(("PASS " if ok else "FAIL ") + message)
	if not ok:
		failures += 1


func _initialize() -> void:
	run.call_deferred()


func bounds(node: Control) -> Rect2:
	return node.get_global_transform() * Rect2(Vector2.ZERO, node.size)


## The first shown label in `node` whose font has a drawn Gridlite field.
func gridlite_label(node: Node) -> Label:
	for l in node.find_children("*", "Label", true, false):
		if l.is_visible_in_tree() and not Gridlite.field_spec(l.get_theme_font("font")).is_empty():
			return l
	return null


func run() -> void:
	var canvas := Control.new()
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.add_child(canvas)
	var page: Control = load("res://ui/tabbed_page/tabbed_page.tscn").instantiate()
	canvas.add_child(page)
	var inspector: Control = load("res://ui/inspect_panel.tscn").instantiate()
	canvas.add_child(inspector)
	var notices: Control = load("res://ui/notices.tscn").instantiate()
	canvas.add_child(notices)
	for screen in [Vector2(1920, 1080), Vector2(1280, 720), Vector2(1280, 1024), Vector2(600, 1000), Vector2(2560, 1080), Vector2(3840, 2160)]:
		var scale := Fit.apply(canvas, screen)
		Fit.fit_inspector(inspector, canvas)
		await process_frame
		await process_frame
		var window := Rect2(Vector2.ZERO, screen).grow(0.1)
		check(scale <= 1.0 and window.encloses(bounds(page.get_node("Center/Page"))), "authored page fits %s" % screen)
		check(bounds(page.get_node("Scrim")).size.is_equal_approx(screen), "scrim covers %s" % screen)
		check(window.encloses(bounds(inspector)), "inspector fits %s" % screen)
		check(window.encloses(bounds(notices)), "notices fit %s" % screen)
		# Gridlite text draws solid below scale 1 (its own glyphs hidden, the cells filled
		# flat) and dotted at 1 (NeonDotField, "Scaled UI").
		var chrome := gridlite_label(page)
		var knobs := NeonDotField._knobs(chrome, true)
		var size: int = chrome.get_theme_font_size("font_size")
		var solid := NeonDotField.solid_at(chrome, size, knobs)
		check(solid == (scale < 1.0), "Gridlite text solid only below scale 1 (%s, scale %.3f)" % [screen, scale])
		check((chrome.self_modulate.a == 0.0) == (scale < 1.0), "label's own dotted glyphs hidden only while solid at %s" % screen)
	canvas.queue_free()
	await process_frame

	var view := View.new()
	view.size = Vector2(1000, 600)
	root.add_child(view)
	view.set_process(false)
	view.baseline_size = Vector2i(256, 128)
	view.raster_size = Vector2i(256, 128)
	view.renderer = RendererStub.new()
	view.reset_framing()
	check(view.screen_to_raster(view.size * 0.5) == Vector2i(128, 64), "overview is centred")
	check(view.screen_to_raster(Vector2(500, 0)).x < 0, "overview letterbox rejects picking")
	view._zoom = 2.0
	view._zoom_to = 2.0
	view._center = Vector2(254, 70)
	view._center_to = view._center
	view._apply_pan()
	for pixel in [Vector2(250, 65), Vector2(2, 80), Vector2(210, 35)]:
		var screen := view.raster_to_screen(pixel)
		check(view.screen_to_raster(screen) == Vector2i(pixel), "screen/world round trip %s" % pixel)
	check(view.pick(view.raster_to_screen(Vector2(2, 80))) == 7 and view.renderer.picked == Vector2i(2, 80), "picking sends baseline coordinates across ring seam")
	view._mouse = Vector2(-1, -1)
	var cursor := Vector2(640, 380)
	var anchored := view._baseline_at(cursor)
	view.zoom_at(3.0, cursor)
	for i in 30:
		view._process(0.016)
	check(view._baseline_at(cursor).distance_to(anchored) < 0.01, "eased zoom holds cursor world point")
	view._drag_by(Vector2(0, 80))
	check(view._center_to.y < view._center.y, "vertical drag changes camera centre")
	var before_keys := view._center_to
	for code in [KEY_W, KEY_D]:
		var key := InputEventKey.new()
		key.keycode = code
		key.physical_keycode = code
		key.pressed = true
		Input.parse_input_event(key)
	Input.flush_buffered_events()
	check(Input.is_key_pressed(KEY_W) and Input.is_key_pressed(KEY_D), "synthetic WASD events reach held key state")
	view._input_pan(0.01)
	check(view._center_to.x > before_keys.x and view._center_to.y < before_keys.y, "WASD pans both axes")
	for code in [KEY_W, KEY_D]:
		var key := InputEventKey.new()
		key.keycode = code
		key.physical_keycode = code
		key.pressed = false
		Input.parse_input_event(key)
	Input.flush_buffered_events()
	view._center_to.y = -1000
	view._apply_pan()
	check(is_equal_approx(view._center_to.y, view._y_range(view._zoom_to).x), "vertical camera stops at map edge")
	view._center = Vector2(254, 64)
	view._center_to = view._center
	view._zoom = 4.0
	view._zoom_to = 4.0
	view.frame_factor = 4.0
	view.frame_view_x = 888
	view.frame_view_y = 192
	view._zoom_anchor_active = false
	view._apply_pan()
	check(view._screen_to_frame(view.size * 0.5).is_equal_approx(Vector2(128, 64)), "detail crop camera maps to source centre")
	var world := view._baseline_at(view.size * 0.5 + Vector2(40, 20))
	var source := view._screen_to_frame(view.size * 0.5 + Vector2(40, 20))
	check(Vector2(fposmod((source.x + view.frame_view_x) / 4.0, 256.0), (source.y + view.frame_view_y) / 4.0).is_equal_approx(world), "detail crop uses shared world transform")
	view._image = Image.create_empty(256, 128, false, Image.FORMAT_RGBA8)
	view._image.fill(Color(0.3, 0.7, 0.2, 1.0))
	var sample := view._screen_to_frame(Vector2(500, 305))
	view._image.set_pixel(int(sample.x), int(sample.y), Color.MAGENTA)
	var capture := view.capture(Vector2i(101, 61))
	check(capture.get_pixel(50, 30) == Color.MAGENTA, "capture shares zoomed crop transform")
	view.input_enabled = false
	check(not view._input_allowed(), "modal input gate stops camera input")
	var wheel := InputEventMouseButton.new()
	wheel.button_index = MOUSE_BUTTON_WHEEL_DOWN
	wheel.pressed = true
	wheel.position = view.size * 0.5
	view._gui_input(wheel)
	check(view._zoom_to == 4.0, "disabled view ignores wheel zoom")
	view.input_enabled = true
	var field := LineEdit.new()
	root.add_child(field)
	field.grab_focus()
	check(not view._input_allowed(), "typing input gate stops camera input")
	field.release_focus()
	field.queue_free()
	view.reset_framing()
	check(view._zoom == 1.0 and view._center.is_equal_approx(Vector2(128, 64)), "reset restores overview")
	view.queue_free()
	print("VIEW FIT TEST: %d failures" % failures)
	quit(0 if failures == 0 else 1)
