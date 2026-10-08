extends SceneTree
## Headless smoke test of the extension:
##   godot --headless --path game --script res://tests/smoke.gd [-- --out DIR] [--config PATH]
## Loads the extension, draws 2 terrain candidates of the smallest size (the desktop
## terrarium, 256 x 128 x 48), founds and runs one, steps about 100 ticks, queries summary, populations and
## one animal, takes a view and checks it holds chunks and organisms, saves to a temp path
## and loads it back. Prints PASS or FAIL lines and exits 0 only if everything passed.

const STEPS := 100
const TIMEOUT_S := 240.0

var session
var state := "start"
var elapsed := 0.0
var failures := 0
var out_dir := ""
var config := ""
var saved_tick := -1
var generation := 0


func check(ok: bool, what: String) -> void:
	print(("PASS " if ok else "FAIL ") + what)
	if not ok:
		failures += 1


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	out_dir = OS.get_user_data_dir().path_join("smoke-save")
	config = ProjectSettings.globalize_path("res://").path_join("../config/desktop/terrarium.toml").simplify_path()
	for i in args.size() - 1:
		if args[i] == "--out":
			out_dir = args[i + 1]
		elif args[i] == "--config":
			config = args[i + 1]
	# A save directory keeps every tick's files and a load reads the newest, so a run
	# saves into an empty one (an earlier run's later tick would be read back).
	preload("res://ui/saves.gd").remove_tree(out_dir)
	check(ClassDB.class_exists("CubariumSession"), "the extension is loaded")
	if not ClassDB.class_exists("CubariumSession"):
		state = "done"
		return
	session = ClassDB.instantiate("CubariumSession")
	var refused: String = session.generate_candidates({"config": config, "seed": "5"}, 2)
	check(refused == "", "two candidates started (%s)" % refused)
	state = "founding"


func _process(delta: float) -> bool:
	elapsed += delta
	if elapsed > TIMEOUT_S and state != "done":
		check(false, "finished within %d s (stuck in %s)" % [TIMEOUT_S, state])
		state = "done"
	match state:
		"founding":
			var c: Array = session.candidates()
			var ready := c.filter(func(x): return x["state"] == "ready")
			var failed := c.filter(func(x): return x["state"] == "failed")
			if failed.size() > 0:
				check(false, "candidates founded: " + str(failed[0]["error"]))
				state = "done"
			elif c.size() == 2 and ready.size() == 2:
				check(c[0]["summary"]["seed"] != c[1]["summary"]["seed"], "the two candidates are two worlds")
				var thumb: Dictionary = session.candidate_view(1)
				check(thumb.get("full", false) and thumb["voxels"].size() > 0, "a candidate has a whole view")
				check(session.promote(0) == "", "candidate 0 is being founded")
				state = "promoted"
		"promoted":
			var f: Dictionary = session.founding()
			if f["state"] == "failed" or f["state"] == "cancelled":
				check(false, "candidate 0 founded: %s" % f.get("error"))
				state = "done"
			var st: Dictionary = session.status()
			if st["running"] and st["generation"] == 1:
				check(session.candidates().size() == 0, "the other candidate is dropped")
				session.pause()
				state = "pausing"
		"pausing":
			var st: Dictionary = session.status()
			if st["paused"]:
				saved_tick = st["tick"]
				session.step(STEPS)
				state = "stepping"
		"stepping":
			var st: Dictionary = session.status()
			if st["tick"] >= saved_tick + STEPS:
				check(st["tick"] == saved_tick + STEPS, "stepped exactly %d ticks" % STEPS)
				state = "querying"
		"querying":
			var summary = session.query("summary", null)
			if summary.has("error") or summary["tick"] < saved_tick + STEPS:
				return false  # the next publication is on its way
			check(summary["width"] == 256 and summary["depth"] == 48, "summary: %dx%dx%d tick %d" % [summary["width"], summary["height"], summary["depth"], summary["tick"]])
			var pops = session.query("populations", null)
			check(pops.get("animals", []).size() > 0, "populations: %d lineages, %d plant species" % [pops["animals"].size(), pops["plants"].size()])
			var animals = session.query("animals", null)
			if animals.size() > 0:
				var one = session.query("animal", {"id": animals[0]["id"]})
				check(not one.has("error") and one["life"] != null, "one animal and its life: #%d %s" % [animals[0]["id"], animals[0]["species"]])
			else:
				check(false, "an animal to query")
			var view: Dictionary = session.take_view()
			var chunk_bytes: int = view["chunk_size"] * view["chunk_size"] * view["chunk_size"] * 3
			check(view["chunk_coords"].size() > 0 and view["voxels"].size() == view["chunk_coords"].size() / 3 * chunk_bytes, "the view holds %d chunks" % (view["chunk_coords"].size() / 3))
			check(view.has("animals") and view["animals"]["ids"].size() > 0, "the view holds %d animals and %d plants" % [view["animals"]["ids"].size(), view["plants"]["ids"].size()])
			var codes: Dictionary = session.query("species", null)
			check(codes["flags"].has("starving") and codes.has("mark_kinds") and codes.has("decay_states") and codes.has("vine_accents"), "new view codes are named")
			check(view["animals"]["riding_host"] is PackedInt64Array and view["animals"]["wall_toward"].size() == 2 * view["animals"]["ids"].size(), "riders and wall directions reach Godot")
			var plants: Dictionary = view["plants"]
			check(plants["wilt"].size() == plants["ids"].size() and plants["ripeness"] is PackedFloat64Array and plants["parcel_fullness"].size() == plants["ids"].size(), "plant wilt and ripeness reach Godot")
			check(plants["foliage_offsets"].size() == plants["ids"].size() + 1 and plants["foliage_offsets"][-1] == plants["foliage_stock"].size() and plants["foliage_stock"].size() == plants["foliage_capacity"].size(), "foliage layer columns are aligned")
			check(view["carcasses"]["pos"].size() == 3 * view["carcasses"]["ids"].size(), "carcass columns reach Godot")
			for key in ["marks", "vines"]:
				var surface: Dictionary = view[key]
				check(surface["offsets"].size() == view["chunk_coords"].size() / 3 + 1 and surface["site"].size() == 3 * surface["offsets"][-1], "%s replacement lists align with chunks" % key)

			check(view.has("sky"), "the view holds the sky (%s, daylight %.2f)" % [view["sky"]["rain"], view["sky"]["daylight"]])
			var events = session.take_events()
			print("     %d life events so far" % events.size())
			session.save(out_dir)
			saved_tick = session.status()["tick"]
			state = "saving"
		"saving":
			for n in session.take_notices():
				if n["kind"] == "saved":
					check(n["meta"]["tick"] == saved_tick, "saved to %s at tick %d" % [out_dir, saved_tick])
					generation = session.status()["generation"]
					session.step(5)
					session.load(out_dir)
					state = "loading"
				elif n["kind"] == "save_failed":
					check(false, "saved: " + n["text"])
					state = "done"
		"loading":
			var st: Dictionary = session.status()
			if st["generation"] > generation and not st["loading"]:
				check(st["tick"] == saved_tick, "loaded back at tick %d" % st["tick"])
				var view: Dictionary = session.take_view()
				check(view["full"], "the first view after the load is whole")
				state = "done"
			for n in session.take_notices():
				if n["kind"] == "load_failed":
					check(false, "loaded: " + n["text"])
					state = "done"
		"done":
			session = null
			print("SMOKE %s (%d failures, %.1f s)" % ["OK" if failures == 0 else "FAILED", failures, elapsed])
			quit(0 if failures == 0 else 1)
			return true
	return false
