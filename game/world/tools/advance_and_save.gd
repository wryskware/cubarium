extends SceneTree
## Load a save, run it on to given ticks and save at each, so view checks can open a world
## at a chosen time of day:
##   godot --headless --path game --script res://world/tools/advance_and_save.gd -- \
##       --load DIR --stops midday:24000,dusk:66000,night:96000 --out-prefix /path/t256-s7
## writes /path/t256-s7-midday, …-dusk, …-night. A stop is an absolute tick (20 ticks a
## second of sim time; on the desktop day clock, hour H of day 1 is tick (H − 8) · 6000).
## The world runs at `--speed X` (default 8) and is saved, still running, at the first frame
## past each stop (a few ticks over at most). Pick a speed the machine sustains with room to
## spare: the session's loop reads no command while it is behind its clock, so a world run
## faster than it can step never takes the save (nor a pause or a slower speed). Nothing is
## drawn; the views and events are taken every frame so the reader keeps up. Exits 0 once
## every stop is saved (1 on a failure).

var session
var state := "loading"
var stops := []   # [[name, tick], …] in order
var prefix := ""
var at := 0
var started := 0
var speed := 8.0


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	var load_dir := ""
	for i in args.size() - 1:
		match args[i]:
			"--load": load_dir = args[i + 1]
			"--out-prefix": prefix = args[i + 1]
			"--speed": speed = float(args[i + 1])
			"--stops":
				for s in args[i + 1].split(","):
					var kv := s.split(":")
					stops.append([kv[0], int(kv[1])])
	if load_dir == "" or prefix == "" or stops.is_empty():
		push_error("advance_and_save: --load DIR --stops name:tick,… --out-prefix P are required")
		quit(2)
		return
	session = ClassDB.instantiate("CubariumSession")
	session.load(load_dir)
	started = Time.get_ticks_msec()
	print("advance_and_save: loading ", load_dir)


func _process(_delta: float) -> bool:
	session.take_view()
	session.take_events()
	match state:
		"loading":
			for n in session.take_notices():
				if n.get("kind", "") == "load_failed":
					push_error("advance_and_save: " + JSON.stringify(n))
					quit(1)
					return true
			var s: Dictionary = session.status()
			if s.get("running", false) and int(s.get("generation", 0)) > 0:
				_run()
		"running":
			session.take_notices()
			var s: Dictionary = session.status()
			if int(s.get("tick", 0)) >= int(stops[at][1]):
				# Saved while running: the save clones the world between two ticks. (A pause
				# here left the loop busy and the save unanswered.)
				var out := "%s-%s" % [prefix, stops[at][0]]
				print("advance_and_save: tick %d at %.1f s, %s, saving %s" % [int(s["tick"]),
					(Time.get_ticks_msec() - started) / 1000.0, JSON.stringify(session.query("clock", null)), out])
				session.save(out)
				state = "saving"
		"saving":
			for n in session.take_notices():
				if n.get("kind", "") == "saved":
					print("advance_and_save: saved ", n.get("path", ""), " at %.1f s" % ((Time.get_ticks_msec() - started) / 1000.0))
					at += 1
					if at >= stops.size():
						quit(0)
						return true
					_run()
				elif n.get("kind", "") == "save_failed":
					push_error("advance_and_save: " + JSON.stringify(n))
					quit(1)
					return true
	return false


func _run() -> void:
	session.set_speed(speed)
	session.resume()
	state = "running"
