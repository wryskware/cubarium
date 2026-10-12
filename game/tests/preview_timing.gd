extends SceneTree
## How long the new-world picker's previews take, cold (the first batch in the process) and
## warm (the next), per card: terrain made, first picture, final picture. A timing, never a
## check. Needs the real renderer; on a private display:
##   CUBARIUM_RENDER_TIMING=1 CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game \
##       --resolution 1920x1080 --script res://tests/preview_timing.gd -- [--size widescreen]
##       [--seed 5] [--batches 2] [--out DIR] [--found]
## Prints `PREVIEW` lines (milliseconds from the batch's request). `--out` saves each card's
## first and final picture as PNGs. `--found` founds card 0 of the last batch and times the
## live world's first and final picture too.

const WorldViews := preload("res://ui/world_views.gd")

var main
var size := "widescreen"
var seed := "5"
var batches := 2
var out := ""
var found := false
var saves_dir := ""


func _initialize() -> void:
	saves_dir = OS.get_user_data_dir().path_join("preview-timing-saves")
	var args := OS.get_cmdline_user_args()
	for i in args.size():
		match args[i]:
			"--size": size = args[i + 1]
			"--seed": seed = args[i + 1]
			"--batches": batches = int(args[i + 1])
			"--out": out = args[i + 1]
			"--found": found = true
	if out != "":
		DirAccess.make_dir_recursive_absolute(out)
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
	await frames(3)
	main._on_menu("new")
	await frames(1)
	main.picker.select(size)
	for b in batches:
		main.picker._seed.text = str(int(seed) + b * 100)
		await batch("cold" if b == 0 else "warm%d" % b)
	if found:
		await found_world()
	finish()


func batch(name: String) -> void:
	var p = main.picker
	var t0 := Time.get_ticks_msec()
	print("PREVIEW %s %s request" % [size, name])
	var refused: String = p.generate()
	if refused != "":
		print("PREVIEW refused: ", refused)
		return
	var ready := {}
	var first := {}
	var final := {}
	var deadline := t0 + 240 * 1000
	while Time.get_ticks_msec() < deadline:
		await process_frame
		var now := Time.get_ticks_msec() - t0
		var list: Array = main.session.candidates()
		var pending := false
		for c in list:
			var i: int = c["index"]
			if c["state"] == "generating":
				pending = true
				continue
			if c["state"] != "ready" or i >= p._cards.size():
				continue
			if not ready.has(i):
				ready[i] = now
			var wv = card_view(i)
			if wv == null:
				pending = true
				continue
			if wv.frames_shown > 0 and not first.has(i):
				first[i] = now
				save(wv, "%s-%s-card%d-first" % [size, name, i])
			if wv.frames_shown > 0 and wv.lighting_ready and not final.has(i):
				final[i] = now
				save(wv, "%s-%s-card%d-final" % [size, name, i])
			if not final.has(i):
				pending = true
		if not pending and list.size() > 0:
			break
	for i in ready.keys():
		print("PREVIEW %s %s card %d: terrain %d ms, first picture %s ms, final %s ms" % [
			size, name, i, ready[i], first.get(i, "none"), final.get(i, "none")])


func found_world() -> void:
	var t0 := Time.get_ticks_msec()
	main.picker._cards[0]["root"].pressed.emit()
	if not await until(func(): return main.has_world(), 280.0):
		print("PREVIEW live: not founded in time")
		return
	var founded := Time.get_ticks_msec() - t0
	var wv = main.world_view
	var gen: int = wv.frame_generation
	var t1 := Time.get_ticks_msec()
	await until(func(): return wv.frames_shown > 0 and wv.frame_generation != gen, 60.0)
	var first := Time.get_ticks_msec() - t1
	save(wv, "%s-live-first" % size)
	await until(func(): return wv.lighting_ready, 60.0)
	var final := Time.get_ticks_msec() - t1
	save(wv, "%s-live-final" % size)
	print("PREVIEW %s live: founded %d ms, then first picture %d ms, final %d ms" % [size, founded, first, final])


func save(wv, name: String) -> void:
	if out == "" or wv._image == null:
		return
	wv._image.save_png(out.path_join(name + ".png"))


func card_view(i: int):
	var holder: Control = main.picker._cards[i]["holder"]
	for child in holder.get_children():
		if child is SubViewportContainer:
			return WorldViews.framed_view(child)
	return null


func frames(n: int) -> void:
	for i in n:
		await process_frame


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
