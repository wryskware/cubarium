extends SceneTree
## Headless test of the new-world picker's thumbnails (mechanisms, never tuned numbers):
##   godot --headless --path game --script res://tests/picker_test.gd [-- --size terrarium]
##       [--seed N] [--bound S]
## Every candidate whose card says Ready gets a thumbnail: its WorldView takes a frame from
## the Rust renderer, the frame is not all one colour, it was drawn with its sky plane (lit,
## not the give-up picture), and the renderer reports no error. A candidate opened large gets
## its frame too.
## Checked for a fresh batch and again after the water slider moves while terrains are still
## being drawn (the batch is replaced under the cards). Each card's time from Ready to its
## first frame and the longest main-thread frame while thumbnails stage are printed (TIMING
## lines) for comparison; they are never pass criteria. `--bound` is how long a ready card
## may wait for its frame before it counts as missing (generous: 120 s). Exits 0 only if
## every check passed.

const WorldViews := preload("res://ui/world_views.gd")

var main
var failures := 0
var size := "terrarium"
var seed := ""
var bound_s := 120.0
var saves_dir := ""


func check(ok: bool, what: String) -> void:
	print(("PASS " if ok else "FAIL ") + what)
	if not ok:
		failures += 1


func _initialize() -> void:
	saves_dir = OS.get_user_data_dir().path_join("picker-test-saves")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--size": size = args[i + 1]
			"--seed": seed = args[i + 1]
			"--bound": bound_s = float(args[i + 1])
	# The app's own pace: the picker polls the session once a frame.
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
	if not WorldViews.is_real():
		print("PICKER TEST: no WorldView scene; nothing to check")
		return finish()
	main._on_menu("new")
	await frames(1)
	main.picker.select(size)
	main.picker._seed.text = seed
	# For comparison: the longest frame with the picker open and nothing being made (the
	# machine's own noise).
	var idle := 0.0
	var last := Time.get_ticks_usec()
	for k in 120:
		await process_frame
		var now := Time.get_ticks_usec()
		idle = maxf(idle, (now - last) / 1000.0)
		last = now
	print("TIMING %s idle: longest frame %.1f ms" % [size, idle])

	# A fresh batch: every ready card gets its thumbnail.
	var refused: String = main.picker.generate()
	check(refused == "", "candidates of %s asked for %s" % [size, refused])
	await every_card_shown("a fresh batch")

	# A ready candidate opened large gets its own frame too (not queued behind thumbnails).
	var first := -1
	for c in main.session.candidates():
		if c["state"] == "ready" and first < 0:
			first = c["index"]
	if first >= 0:
		var at := Time.get_ticks_msec()
		main.picker.open_large(first)
		var large_drawn := await until(_large_drawn, bound_s)
		var lv = main.picker.large_view()
		check(large_drawn and colours(lv._image) > 1,
			"candidate %d opened large shows a picture (%s)" % [first, lv.problem() if lv != null and lv.has_method("problem") else ""])
		print("TIMING %s open large: first frame %d ms" % [size, Time.get_ticks_msec() - at])
		main.picker.close_large()
		await frames(1)

	# The slider moves while terrains are still being drawn: the batch is replaced under the
	# cards, and every card of the new batch still gets its thumbnail.
	main.picker._seed.text = ""
	main.picker.generate()
	var drawing := await until(_drawing, 10.0)
	await until(func(): return main.session.candidates().any(func(c): return c["state"] == "ready"), 240.0)
	var still_drawing: bool = main.session.candidates().any(func(c): return c["state"] == "generating")
	print("     moving the slider with %s" % [main.session.candidates().map(func(c): return c["state"])])
	main.picker._water.value = (int(main.picker._water.value) + 1) % 3
	check(drawing, "the second batch was being drawn when the slider moved (still drawing: %s)" % still_drawing)
	await every_card_shown("after the water slider moved mid-draw")
	finish()


## Wait for the batch to end, then for every ready card's thumbnail; check each, and print
## the times.
func every_card_shown(what: String) -> void:
	var p = main.picker
	var shown_ms := {}
	var longest_ms := 0.0
	var last := Time.get_ticks_usec()
	var deadline := Time.get_ticks_msec() + 200 * 1000
	while Time.get_ticks_msec() < deadline:
		await process_frame
		var now := Time.get_ticks_usec()
		longest_ms = maxf(longest_ms, (now - last) / 1000.0)
		last = now
		var list: Array = main.session.candidates()
		var all_done := list.size() > 0 and list.all(func(c): return c["state"] != "generating")
		var waiting := false
		for c in list:
			var i: int = c["index"]
			if c["state"] != "ready" or i >= p._cards.size():
				continue
			var wv = card_view(i)
			if wv != null and wv.frames_shown > 0:
				if not shown_ms.has(i):
					shown_ms[i] = Time.get_ticks_msec()
				continue
			if not p._ready_ms.has(i) or Time.get_ticks_msec() - p._ready_ms[i] < bound_s * 1000.0:
				waiting = true
		if all_done and not waiting:
			break
	var list: Array = main.session.candidates()
	var ready := list.filter(func(c): return c["state"] == "ready")
	check(ready.size() > 0, "%s: some terrains drawn (%s)" % [what, list.map(func(c): return c["state"])])
	var times := []
	for c in ready:
		var i: int = c["index"]
		var wv = card_view(i)
		var renderer_error: String = wv.renderer.error() if wv != null and wv.renderer != null else ""
		check(wv != null and wv.frames_shown > 0,
			"%s: ready card %d shows a frame within %d s of Ready (%s)" % [what, i, int(bound_s), renderer_error])
		check(renderer_error == "", "%s: card %d's renderer reports no error" % [what, i])
		check(wv != null and wv.renderer != null and not wv.renderer.stats().get("unlit", false),
			"%s: card %d's thumbnail was drawn with its sky plane (lit)" % [what, i])
		check(p._cards[i].get("ready", false) and not p._cards[i]["open"].disabled,
			"%s: card %d can be opened" % [what, i])
		if wv != null and wv._image != null:
			check(colours(wv._image) > 1, "%s: card %d's thumbnail is not all one colour" % [what, i])
		if shown_ms.has(i) and p._ready_ms.has(i):
			times.append("%d: %d ms" % [i, shown_ms[i] - p._ready_ms[i]])
		else:
			times.append("%d: none" % i)
	print("TIMING %s %s: Ready to first frame %s; longest frame %.1f ms" % [size, what, ", ".join(times), longest_ms])


func _large_drawn() -> bool:
	var lv = main.picker.large_view()
	return lv != null and lv.frames_shown > 0


func _drawing() -> bool:
	return main.picker._cards.size() > 0 \
		and main.session.candidates().any(func(c): return c["state"] == "generating")


## Card `i`'s WorldView, or null.
func card_view(i: int):
	var holder: Control = main.picker._cards[i]["holder"]
	for child in holder.get_children():
		if child is SubViewportContainer:
			return WorldViews.framed_view(child)
	return null


## Distinct colours over a sample grid of `img` (up to two: enough to tell a picture from a
## blank).
static func colours(img: Image) -> int:
	var first := img.get_pixel(0, 0)
	var step := maxi(1, mini(img.get_width(), img.get_height()) / 64)
	for y in range(0, img.get_height(), step):
		for x in range(0, img.get_width(), step):
			if not img.get_pixel(x, y).is_equal_approx(first):
				return 2
	return 1


func finish() -> void:
	main.picker.leave()
	print("PICKER TEST %s: %d failure(s)" % [size, failures])
	preload("res://ui/saves.gd").remove_tree(saves_dir)
	DirAccess.remove_absolute(saves_dir + ".cfg")
	quit(1 if failures > 0 else 0)


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
