extends Node
## **The dev scene** for `WorldView` (the Rust renderer's): a world shown through it, for
## stills, picks and frame times. Never a window on the desktop: run it through
## `scripts/hidden.sh` (Xvfb), or `scripts/hidden.sh --hyprland` for frame times.
##
##   scripts/hidden.sh godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- [options]
##
## A world, one of:
##   --load DIR               load a save (load a copy: the world runs on)
##   --candidate SEED         found one candidate of --config (default the desktop
##                            terrarium) and show it through show_view
## Then:
##   --ticks N                let the world run N ticks, then pause (default 40)
##   --still PATH             the screen once paused and drawn
##   --raster PATH            the renderer's raster as it arrived
##   --pan F                  then pan F view widths round the ring, and --panned PATH
##   --pick PATH              then pick an animal at its drawn place (prints the check),
##                            highlight it and save the screen to PATH
##   --capture PATH           capture(320x160)
##   --perf S                 instead of pausing: print the cost line every second for S
##                            seconds (`--pan-during-perf` keeps panning), then quit

const DEFAULT_CONFIG := "../config/desktop/terrarium.toml"
const SETTLE_S := 0.8

var session
var view
var opts := {}
var state := "start"
var clock := 0.0
var _mark := 0.0
var _start_tick := -1
var _frames_at_pause := -1
var _perf_lines := 0


func _ready() -> void:
	view = $WorldView
	var args := OS.get_cmdline_user_args()
	var i := 0
	while i < args.size():
		var a: String = args[i]
		if a in ["--pan-during-perf"]:
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
	elif opts.has("candidate"):
		var cfg := ProjectSettings.globalize_path("res://").path_join(str(opts.get("config", DEFAULT_CONFIG))).simplify_path()
		var refused: String = session.generate_candidates({"config": cfg, "seed": int(opts["candidate"])}, 1)
		if refused != "":
			_quit("refused: " + refused, 1)
		state = "founding"
	else:
		_quit("give --load DIR or --candidate SEED", 1)


func _process(delta: float) -> void:
	clock += delta
	if clock > 600.0:
		_quit("timed out in state " + state, 1)
	match state:
		"loading":
			if session.status()["running"] and view.frames_shown > 0:
				_start_tick = int(session.status()["tick"])
				state = "perf" if opts.has("perf") else "running"
				_mark = clock
				print("dev_view: shown at tick %d, raster %s" % [_start_tick, view.raster_size])
		"founding":
			var c: Array = session.candidates()
			if c.size() > 0 and c[0]["state"] == "ready":
				view.show_view(session.candidate_view(0))
				state = "candidate"
			elif c.size() > 0 and c[0]["state"] == "failed":
				_quit("candidate failed: " + str(c[0]["error"]), 1)
		"candidate":
			if view.frames_shown > 0:
				_mark = clock
				state = "settle"
		"running":
			if int(session.status()["tick"]) >= _start_tick + int(opts.get("ticks", "40")):
				session.pause()
				_mark = clock
				state = "settle"
		"settle":
			if clock - _mark >= SETTLE_S:
				_stills()
		"panning":
			if clock - _mark >= 1.5:
				_save_screen(str(opts["panned"]))
				state = "picking" if opts.has("pick") else "done"
		"picking":
			_pick()
		"picked":
			if clock - _mark >= SETTLE_S:
				_save_screen(str(opts["pick"]))
				state = "done"
		"perf":
			if opts.has("pan-during-perf"):
				view.pan_by(Vector2(0.25 * delta, 0.0))
			if clock - _mark >= 1.0 * (_perf_lines + 1):
				_perf_lines += 1
				print("dev_view: " + view.perf_line())
			if _perf_lines >= int(opts["perf"]):
				state = "done"
		"done":
			if view.renderer != null and view.renderer.error() != "":
				print("dev_view: renderer error: " + view.renderer.error())
			_quit("done", 0)


func _stills() -> void:
	if opts.has("still"):
		_save_screen(str(opts["still"]))
	if opts.has("raster") and view._image != null:
		view._image.save_png(str(opts["raster"]))
		print("dev_view: raster " + str(opts["raster"]))
	if opts.has("capture"):
		view.capture(Vector2i(320, 160)).save_png(str(opts["capture"]))
	print("dev_view: " + view.perf_line())
	if opts.has("pan"):
		view.pan_by(Vector2(float(opts["pan"]), 0.0))
		_mark = clock
		state = "panning"
	elif opts.has("pick"):
		state = "picking"
	else:
		state = "done"


func _pick() -> void:
	# Every animal drawn on screen, picked at the middle of its drawn cells: the middle can
	# fall on a nearer body or rock, so the count says how many came back as themselves.
	var animals = session.query("animals", null)
	var best := -1
	var best_body := -1.0
	var shown := 0
	var hits := 0
	if animals is Array:
		for a in animals:
			var row: Dictionary = a["row"] if a.has("row") else a
			var id := int(row.get("id", -1))
			var p: Vector2 = view.animal_screen(id)
			if p.x < 0.0 or not Rect2(Vector2(40, 40), view.size - Vector2(80, 80)).has_point(p):
				continue
			shown += 1
			if view.pick(p) == id:
				hits += 1
				# The biggest body picked, so the highlight shows.
				if float(row.get("body", 0.0)) > best_body:
					best = id
					best_body = float(row.get("body", 0.0))
	print("dev_view: pick: %d of %d animals on screen picked at their drawn middles" % [hits, shown])
	if best < 0:
		state = "done"
		return
	view.set_highlight(best)
	print("dev_view: highlighted animal %d at %s" % [best, view.animal_screen(best)])
	_mark = clock
	state = "picked"


func _save_screen(path: String) -> void:
	var img := get_viewport().get_texture().get_image()
	img.save_png(path)
	print("dev_view: still " + path)


func _quit(why: String, code: int) -> void:
	print("dev_view: " + why)
	view.detach()
	get_tree().quit(code)
