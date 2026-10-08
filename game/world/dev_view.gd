extends Node
## **The dev scene** for `WorldView`: a world shown through it, for checks and pictures.
##
##   godot --path game res://world/dev_view.tscn -- [options]     (through scripts/hidden.sh)
##
## A world, one of:
##   --seed N [--config PATH]   found one (default config ../config/desktop/terrarium.toml)
##   --load DIR                 load a save (much faster; see world/tools/found_and_save.gd)
##   --candidate                found it as a candidate and show it with show_view (static)
## Checks:
##   --shots DIR [--prefix P]   once shown: DIR/P-framing.png, then a pan along the ring,
##                              DIR/P-panned.png and DIR/P-capture.png (capture() at 640x360),
##                              then quit
##   --pan-demo                 once shown: pan right round the ring and back, then quit
##   --record DIR               save every frame once shown as DIR/f00000.png (run with
##                              `--fixed-fps 30` for frames at real-time spacing; Godot's own
##                              --write-movie ignores --resolution)
##   --perf S                   once shown: print perf lines for S seconds, then quit
##   --pan-during-perf          keep panning while measuring
##   --speed X                  the sim's speed
## Keys: the view's own pan keys; Esc quits.

const DEFAULT_CONFIG := "../config/desktop/terrarium.toml"
const SETTLE_S := 2.0

var session
## The WorldView (untyped: a project never opened in the editor has no class cache).
var view
var opts := {}
var state := "founding"
var shown_at := -1.0
var clock := 0.0
var _stage := 0
var _stage_at := 0.0
var _perf := []
var _recorded := 0


func _ready() -> void:
	view = $WorldView
	var args := OS.get_cmdline_user_args()
	var i := 0
	while i < args.size():
		var a: String = args[i]
		if a in ["--candidate", "--pan-demo", "--pan-during-perf"]:
			opts[a.substr(2)] = true
		elif i + 1 < args.size():
			opts[a.substr(2)] = args[i + 1]
			i += 1
		i += 1
	session = CubariumSession.new()
	if opts.has("load"):
		session.load(str(opts["load"]))
		view.attach(session)
		state = "loading"
		print("dev_view: loading ", opts["load"])
		return
	var config := str(opts.get("config", DEFAULT_CONFIG))
	if not config.is_absolute_path():
		config = ProjectSettings.globalize_path("res://").path_join(config).simplify_path()
	var spec := {"config": config}
	if opts.has("seed"):
		spec["seed"] = int(opts["seed"])
	var refused: String = session.generate_candidates(spec, 1)
	if refused != "":
		push_error("dev_view: " + refused)
		get_tree().quit(1)
		return
	print("dev_view: founding from ", config)


func _process(delta: float) -> void:
	clock += delta
	match state:
		"founding":
			var c: Array = session.candidates()
			if c.size() > 0 and c[0]["state"] == "failed":
				push_error("dev_view: " + str(c[0]["error"]))
				get_tree().quit(1)
			elif c.size() > 0 and c[0]["state"] == "ready":
				if opts.has("candidate"):
					view.show_view(session.candidate_view(0))
				else:
					session.promote(0)
					view.attach(session)
				_shown()
		"loading":
			var s: Dictionary = session.status()
			if s.get("running", false) and int(s.get("generation", 0)) > 0 and view.dims.x > 0:
				_shown()
		"shown":
			_checks(delta)


func _shown() -> void:
	state = "shown"
	shown_at = clock
	if opts.has("speed"):
		session.set_speed(float(opts["speed"]))
	print("dev_view: shown, dims ", view.dims)


func _checks(_delta: float) -> void:
	var since := clock - shown_at
	if opts.has("record"):
		var img := get_viewport().get_texture().get_image()
		img.save_png(str(opts["record"]).path_join("f%05d.png" % _recorded))
		_recorded += 1
	if opts.has("shots"):
		_shots(since)
	elif opts.has("pan-demo"):
		_pan_demo(since)
	if opts.has("perf"):
		if since > SETTLE_S:
			_perf.append([Performance.get_monitor(Performance.TIME_FPS), view._mesh_ms, view._mesh_chunks, view._frame_ms])
			if opts.has("pan-during-perf") and fmod(since, 4.0) < get_process_delta_time():
				view.pan_by(Vector2(0.3, 0.0))
		if since > SETTLE_S + float(opts["perf"]):
			_report_perf()
			get_tree().quit(0)


func _report_perf() -> void:
	var fps := []
	var mesh := []
	var proc := []
	var frame_ms := []
	for p in _perf:
		fps.append(p[0])
		mesh.append(p[1])
		proc.append(p[3])
	fps.sort()
	mesh.sort()
	proc.sort()
	var med := func(a: Array): return a[a.size() / 2] if a.size() > 0 else 0.0
	var top := func(a: Array): return a[int(a.size() * 0.99)] if a.size() > 0 else 0.0
	var mesh_frames := _perf.filter(func(p): return p[2] > 0)
	var mesh_cost := []
	for p in mesh_frames:
		mesh_cost.append(p[1])
	mesh_cost.sort()
	print("dev_view perf: dims %s frames %d  fps median %.1f (min %.1f)  ms/frame %.2f  process median %.2f ms p99 %.2f ms  mesh on %d frames: median %.2f ms p99 %.2f ms max %.2f ms" % [
		view.dims, _perf.size(), med.call(fps), fps[0] if fps.size() > 0 else 0.0,
		1000.0 / maxf(med.call(fps), 0.001), med.call(proc), top.call(proc),
		mesh_frames.size(), med.call(mesh_cost), top.call(mesh_cost), mesh_cost.max() if mesh_cost.size() > 0 else 0.0])
	print("dev_view perf: draw calls %d, objects %d, primitives %d, video mem %.0f MB" % [
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0])


func _shots(since: float) -> void:
	var dir := str(opts["shots"])
	var prefix := str(opts.get("prefix", "view"))
	match _stage:
		0:
			if since > SETTLE_S:
				_save(dir.path_join(prefix + "-framing.png"))
				view.pan_by(Vector2(0.37, 0.0))
				_stage = 1
				_stage_at = since
		1:
			if since - _stage_at > 1.5:
				_save(dir.path_join(prefix + "-panned.png"))
				var img: Image = view.capture(Vector2i(640, 360))
				img.save_png(dir.path_join(prefix + "-capture.png"))
				print("dev_view: saved ", dir.path_join(prefix + "-capture.png"), " ", img.get_size())
				_stage = 2
				get_tree().quit(0)


func _pan_demo(since: float) -> void:
	# Settle, pan a third of the ring right, hold, pan back past the start, hold, home.
	var steps := [[SETTLE_S, Vector2(0.33, 0.0)], [SETTLE_S + 3.0, Vector2(0.33, 0.0)],
		[SETTLE_S + 6.0, Vector2(-0.8, 0.0)], [SETTLE_S + 10.0, Vector2(0.14, 0.0)]]
	if _stage < steps.size() and since > steps[_stage][0]:
		view.pan_by(steps[_stage][1])
		_stage += 1
	elif _stage == steps.size() and since > SETTLE_S + 13.0:
		get_tree().quit(0)


func _save(path: String) -> void:
	var img := get_viewport().get_texture().get_image()
	img.save_png(path)
	print("dev_view: saved ", path, " ", img.get_size())


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.keycode == KEY_ESCAPE:
		get_tree().quit(0)
