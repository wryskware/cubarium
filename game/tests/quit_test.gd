extends SceneTree
## Headless test that quitting is clean wherever it happens:
##   godot --headless --path game --script res://tests/quit_test.gd -- \
##       --saves DIR --at draw|thumbnail|founding|running [--size terrarium] [--seed 5] [--run-s 5] [--via menu|tree] [--timeout 240]
## Runs main.tscn and quits while terrain candidates are being drawn (`draw`), while a
## drawn one's thumbnail is being staged with others waiting their turn (`thumbnail`: three
## candidates), while the picked one is being founded (`founding`), or with its world
## running for `--run-s` seconds (`running`), through the menu's Quit (`menu`, the default) or the SceneTree's own quit,
## which is what closing the window does (`tree`). The extension must stop and join every
## thread it started before Godot unloads it, so **the process must exit 0**: a thread left
## running in the unloaded library is a SIGSEGV (139) after the last line below. The
## wrapper checks the exit code; this script prints `QUIT_AT_MS <unix ms>` just before
## quitting, for timing how long the exit takes.

var timeout_s := 240.0

var main
var started := 0
var saves_dir := ""
var at := "running"
var size_id := "terrarium"
var seed_text := "5"
var run_s := 5.0
var via := "menu"


func _initialize() -> void:
	started = Time.get_ticks_msec()
	saves_dir = OS.get_user_data_dir().path_join("quit-test-saves")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--saves": saves_dir = args[i + 1]
			"--at": at = args[i + 1]
			"--size": size_id = args[i + 1]
			"--seed": seed_text = args[i + 1]
			"--run-s": run_s = float(args[i + 1])
			"--via": via = args[i + 1]
			"--timeout": timeout_s = float(args[i + 1])
	run()


func run() -> void:
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves_dir
	main.settings_path = saves_dir + ".cfg"
	main.quit_hook = func(): _quit_now()
	root.add_child(main)
	await frames(3)
	main.picker.select(size_id)
	main.picker._seed.text = seed_text
	var refused: String = main.picker.generate(3 if at == "thumbnail" else 1)
	if refused != "":
		print("QUIT TEST: generate refused: " + refused)
		quit(2)
		return
	if at == "draw":
		await until(func(): return main.session.workers() > 0, 10.0)
		await frames(5)
		print("QUIT TEST: quitting mid-draw (%d worker(s), candidates %s)" % [main.session.workers(), main.session.candidates().map(func(c): return c["state"])])
		_quit()
		return
	if at == "thumbnail":
		if not await until(_thumbnail_staged, timeout_s):
			print("QUIT TEST: no thumbnail was being staged")
			quit(2)
			return
		print("QUIT TEST: quitting while a thumbnail is staged (candidates %s)" % [main.session.candidates().map(func(c): return c["state"])])
		_quit()
		return
	if not await until(func(): return main.picker.ready_count() == 1, timeout_s):
		print("QUIT TEST: no candidate drawn")
		quit(2)
		return
	print("QUIT TEST: drawn (%d s)" % elapsed())
	var problem: String = main.found_candidate(0)
	if problem != "":
		print("QUIT TEST: founding refused: " + problem)
		quit(2)
		return
	await until(func(): return main.session.founding()["state"] == "founding", 10.0)
	if at == "founding":
		await frames(10)
		print("QUIT TEST: quitting mid-founding (%s)" % main.session.founding()["state"])
		_quit()
		return
	if not await until(func(): return main.has_world(), timeout_s):
		print("QUIT TEST: the founding did not finish (%s)" % main.session.founding())
		quit(2)
		return
	print("QUIT TEST: founded and running (%d s)" % elapsed())
	var until_ms := Time.get_ticks_msec() + int(run_s * 1000.0)
	await until(func(): return Time.get_ticks_msec() >= until_ms, run_s + 1.0)
	print("QUIT TEST: quitting a running world (tick %s)" % main.session.status().get("tick"))
	_quit()


## A ready card's thumbnail has its renderer and no frame yet.
func _thumbnail_staged() -> bool:
	for card in main.picker._cards:
		var wv = card.get("view")
		if wv != null and wv.renderer != null and wv.frames_shown == 0:
			return true
	return false


func _quit() -> void:
	if via == "menu":
		main._on_menu("quit")
	else:
		_quit_now()


func _quit_now() -> void:
	print("QUIT_AT_MS %d" % int(Time.get_unix_time_from_system() * 1000.0))
	quit(0)


func elapsed() -> int:
	return (Time.get_ticks_msec() - started) / 1000


func frames(n: int) -> void:
	for i in n:
		await process_frame


func until(cond: Callable, timeout_s: float) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not cond.call():
		if Time.get_ticks_msec() > deadline or elapsed() > timeout_s:
			return false
		await process_frame
	return true
