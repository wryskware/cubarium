extends SceneTree
## Small real renderer smoke and screenshots. Run on Xvfb via scripts/hidden.sh through labd.
## godot --path game --resolution 1000x1000 --script res://tests/zoom_shots.gd -- --out /tmp/cubarium-ui-zoom

const View := preload("res://world/world_view.gd")
const Fit := preload("res://ui/window_fit.gd")
var failures := 0
var out := "/tmp/cubarium-ui-zoom"
var session
var view
var canvas: Control
var page: Control
var inspector: Control


func _initialize() -> void:
	Engine.max_fps = 60
	var args := OS.get_cmdline_user_args()
	var at := args.find("--out")
	if at >= 0 and at + 1 < args.size():
		out = args[at + 1]
	run.call_deferred()


func check(ok: bool, message: String) -> void:
	print(("PASS " if ok else "FAIL ") + message)
	if not ok:
		failures += 1


func until(condition: Callable, timeout_s := 90.0) -> bool:
	var end := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not condition.call():
		if Time.get_ticks_msec() >= end:
			return false
		await process_frame
	return true


func run() -> void:
	if DisplayServer.get_name() == "headless":
		print("zoom_shots needs a hidden display for shader rendering")
		quit(1)
		return
	DirAccess.make_dir_recursive_absolute(out)
	session = CubariumSession.new()
	var config := ProjectSettings.globalize_path("res://../config/desktop/terrarium.toml").simplify_path()
	var refused: String = session.generate_candidates({"config": config, "size": [96, 64, 24], "terrain": "terrarium-small", "seed": "7"}, 1)
	check(refused == "", "small candidate generation accepted: %s" % refused)
	if refused != "":
		return finish()
	var ready := await until(func():
		var list: Array = session.candidates()
		return not list.is_empty() and list[0]["state"] in ["ready", "failed"], 90.0)
	check(ready and session.candidates()[0]["state"] == "ready", "small candidate ready")
	if not ready or session.candidates()[0]["state"] != "ready":
		return finish()
	view = View.new()
	view.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.add_child(view)
	view.show_view(session.candidate_ref(0))
	check(await until(func(): return view.frames_shown > 0), "overview frame rendered")
	if view.frames_shown == 0:
		print("RENDERER ERROR ", view.problem())
		return finish()
	var previous_s: int = view.frame_s
	check(view.frame_factor == 1.0, "overview uses baseline detail")
	check(view.renderer.error() == "", "overview renderer has no error")
	await shot("01-square-overview")
	for zoom in [2.0, 4.0]:
		view.zoom_at(zoom, view.size * 0.5)
		var before: int = view.frames_shown
		check(await until(func(): return is_equal_approx(view._zoom, zoom) and view.frames_shown > before and view.frame_factor >= zoom and view.frame_s > previous_s), "zoom %.0fx increases detail tier" % zoom)
		check(view.renderer.error() == "", "zoom %.0fx renderer has no error" % zoom)
		previous_s = view.frame_s
		print("DETAIL zoom=", zoom, " s=", view.frame_s, " base_s=", view.frame_base_s, " factor=", view.frame_factor, " baseline=", view.baseline_size, " crop=", view.raster_size)
		await shot("02-square-zoom%d" % zoom)
	var before_pan: Vector2 = view._center
	view.pan_by(Vector2(0.2, 0.15))
	await until(func(): return view._center.distance_to(view._center_to) < 0.05)
	check(absf(view._center.x - before_pan.x) > 0.1 and absf(view._center.y - before_pan.y) > 0.1, "camera pans both axes")
	check(view.renderer.error() == "", "panned renderer has no error")
	await shot("03-square-panned")
	# Render the same real world under the actual authored UI at two non-16:9 shapes.
	var layer := CanvasLayer.new()
	layer.layer = 10
	root.add_child(layer)
	canvas = Control.new()
	canvas.mouse_filter = Control.MOUSE_FILTER_IGNORE
	layer.add_child(canvas)
	page = load("res://ui/tabbed_page/tabbed_page.tscn").instantiate()
	canvas.add_child(page)
	page.get_node("World").visible = false
	inspector = load("res://ui/inspect_panel.tscn").instantiate()
	canvas.add_child(inspector)
	inspector.visible = false
	for screen in [Vector2i(1000, 1000), Vector2i(720, 1000)]:
		root.size = screen
		for i in 3:
			await process_frame
		Fit.apply(canvas, Vector2(root.size))
		Fit.fit_inspector(inspector, canvas)
		page.visible = true
		await shot("04-ui-%dx%d" % [screen.x, screen.y])
		page.visible = false
		inspector.visible = true
		await shot("05-inspect-%dx%d" % [screen.x, screen.y])
		inspector.visible = false
		await shot("06-world-%dx%d" % [screen.x, screen.y])
	finish()


func shot(name: String) -> void:
	for i in 3:
		await process_frame
	await RenderingServer.frame_post_draw
	var image := root.get_texture().get_image()
	image.save_png(out.path_join(name + ".png"))
	print("SHOT ", out.path_join(name + ".png"))


func finish() -> void:
	if view != null:
		view.detach()
	session = null
	print("ZOOM SHOTS: %d failures; %s" % [failures, out])
	quit(0 if failures == 0 else 1)
