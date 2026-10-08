extends SceneTree
## Found one world and save it, so later view checks can `load` it instead of founding:
##   godot --headless --path game --script res://world/tools/found_and_save.gd -- \
##       --config ../config/desktop/terrarium.toml --seed 7 --out /path/to/save [--ticks N]
## Prints the populations and the view's size, then exits 0 once saved (1 on a failure).
## Founding the wide 640 x 128 x 96 world takes minutes: run it detached.

var session
var state := "founding"
var out_dir := ""
var ticks := 0
var started := 0


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	var config := "../config/desktop/terrarium.toml"
	var spec := {}
	for i in args.size() - 1:
		match args[i]:
			"--config": config = args[i + 1]
			"--seed": spec["seed"] = int(args[i + 1])
			"--out": out_dir = args[i + 1]
			"--ticks": ticks = int(args[i + 1])
	if not config.is_absolute_path():
		config = ProjectSettings.globalize_path("res://").path_join(config).simplify_path()
	spec["config"] = config
	if out_dir == "":
		push_error("found_and_save: --out DIR is required")
		quit(2)
		return
	session = ClassDB.instantiate("CubariumSession")
	var refused: String = session.generate_candidates(spec, 1)
	if refused != "":
		push_error("found_and_save: " + refused)
		quit(1)
		return
	started = Time.get_ticks_msec()
	print("found_and_save: founding from ", config)


func _process(_delta: float) -> bool:
	match state:
		"founding":
			var c: Array = session.candidates()
			if c.size() > 0 and c[0]["state"] == "failed":
				push_error("found_and_save: " + str(c[0]["error"]))
				quit(1)
				return true
			if c.size() > 0 and c[0]["state"] == "ready":
				print("found_and_save: founded in %.1f s: %s" % [(Time.get_ticks_msec() - started) / 1000.0, c[0]["label"]])
				session.promote(0)
				if ticks > 0:
					session.step(ticks)
				state = "running"
		"running":
			var s: Dictionary = session.status()
			if s.get("running", false) and int(s.get("tick", 0)) >= ticks:
				var v: Dictionary = session.take_view()
				var a: Dictionary = v.get("animals", {})
				var p: Dictionary = v.get("plants", {})
				print("found_and_save: tick %d, dims %s, %d chunks, %d animals, %d plants" % [
					int(s["tick"]), v.get("dims"), v.get("chunk_coords", PackedInt32Array()).size() / 3,
					a.get("ids", PackedInt64Array()).size(), p.get("ids", PackedInt64Array()).size()])
				print("found_and_save: ", JSON.stringify(session.query("populations", null)))
				session.pause()
				session.save(out_dir)
				state = "saving"
		"saving":
			for n in session.take_notices():
				if n.get("kind", "") == "saved":
					print("found_and_save: saved ", out_dir)
					quit(0)
					return true
				if n.get("kind", "") == "save_failed":
					push_error("found_and_save: " + JSON.stringify(n))
					quit(1)
					return true
	return false
