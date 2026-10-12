extends SceneTree
## How soon after launch the app shows its first preview and live world, and how smooth the
## hub's frames are meanwhile (the pipeline warm-up compiles in the background then). A
## timing, never a check. Needs the real renderer; on a private display:
##   CUBARIUM_RENDER_TIMING=1 scripts/hidden.sh godot --path game --gpu-index 1 \
##       --resolution 1920x1080 --script res://tests/warm_timing.gd -- \
##       [--hub-s 20] [--size widescreen] [--seed 5] [--found]
## Point `__GL_SHADER_DISK_CACHE_PATH` and `CUBARIUM_PIPELINE_CACHE` at empty directories for
## a cold start. Prints `WARM` lines, milliseconds since the engine started.

const WorldViews := preload("res://ui/world_views.gd")

var main
var size := "widescreen"
var seed := "5"
var hub_s := 0.0
var found := false
var saves_dir := ""


func _initialize() -> void:
	saves_dir = OS.get_user_data_dir().path_join("warm-timing-saves")
	var args := OS.get_cmdline_user_args()
	for i in args.size():
		match args[i]:
			"--size": size = args[i + 1]
			"--seed": seed = args[i + 1]
			"--hub-s": hub_s = float(args[i + 1])
			"--found": found = true
	Engine.max_fps = 60
	run()


func run() -> void:
	preload("res://ui/saves.gd").remove_tree(saves_dir)
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves_dir
	main.settings_path = saves_dir + ".cfg"
	main.autoload = false
	main.quit_hook = func(): pass
	root.add_child(main)
	print("WARM hub up at %d ms" % Time.get_ticks_msec())
	if hub_s > 0.0:
		await frame_times("hub", hub_s, func(): return false)
	main._on_menu("new")
	await process_frame
	main.picker.select(size)
	main.picker._seed.text = seed
	var t0 := Time.get_ticks_msec()
	var refused: String = main.picker.generate()
	if refused != "":
		print("WARM refused: ", refused)
		finish()
		return
	print("WARM previews asked at %d ms" % t0)
	var first := [-1]
	await frame_times("picker", 240.0, func():
		var wv = card_view(0)
		if wv != null and wv.frames_shown > 0 and first[0] < 0:
			first[0] = Time.get_ticks_msec()
		return first[0] >= 0)
	print("WARM first preview picture at %d ms (%d ms after asking)" % [first[0], first[0] - t0])
	if found:
		await found_world()
	finish()


## Frame times of the main thread until `done` or `seconds` pass.
func frame_times(what: String, seconds: float, done: Callable) -> void:
	var deltas: Array[float] = []
	var slow: Array[String] = []
	var deadline := Time.get_ticks_msec() + int(seconds * 1000.0)
	var last := Time.get_ticks_usec()
	while Time.get_ticks_msec() < deadline and not done.call():
		await process_frame
		var now := Time.get_ticks_usec()
		deltas.append((now - last) / 1000.0)
		if deltas[-1] > 50.0:
			slow.append("%.0f@%d(unix %d)" % [deltas[-1], Time.get_ticks_msec(), int(Time.get_unix_time_from_system() * 1000.0)])
		last = now
	if deltas.is_empty():
		return
	var sorted := deltas.duplicate()
	sorted.sort()
	var over := 0
	var total := 0.0
	for d in deltas:
		total += d
		if d > 25.0:
			over += 1
	print("WARM %s frames: %d, mean %.1f ms, p99 %.1f ms, max %.1f ms, over 25 ms: %d" % [
		what, deltas.size(), total / deltas.size(), sorted[int(sorted.size() * 0.99)], sorted[-1], over])
	print("WARM %s frames over 50 ms (ms@at): %s" % [what, " ".join(slow)])


func found_world() -> void:
	var t0 := Time.get_ticks_msec()
	main.picker._cards[0]["root"].pressed.emit()
	if not await until(func(): return main.has_world(), 250.0):
		print("WARM live: not founded in time")
		return
	var founded := Time.get_ticks_msec()
	var wv = main.world_view
	var gen: int = wv.frame_generation
	await until(func(): return wv.frames_shown > 0 and wv.frame_generation != gen, 120.0)
	var first := Time.get_ticks_msec()
	print("WARM live: founded in %d ms, first live frame %d ms after that (%d ms after asking)" % [
		founded - t0, first - founded, first - t0])


func card_view(i: int):
	if main.picker._cards.size() <= i:
		return null
	var holder: Control = main.picker._cards[i]["holder"]
	for child in holder.get_children():
		if child is SubViewportContainer:
			return WorldViews.framed_view(child)
	return null


func until(cond: Callable, timeout_s: float) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not cond.call():
		if Time.get_ticks_msec() > deadline:
			return false
		await process_frame
	return true


func finish() -> void:
	main.picker.leave()
	preload("res://ui/saves.gd").remove_tree(saves_dir)
	DirAccess.remove_absolute(saves_dir + ".cfg")
	quit(0)
