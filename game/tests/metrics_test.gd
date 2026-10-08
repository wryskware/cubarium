extends SceneTree
## Headless test of the dev metrics panel's mechanisms (never tuned numbers):
##   godot --headless --path game --script res://tests/metrics_test.gd -- --open SAVE
## Opens the save (a slot directory with meta.json, e.g. from `cubarium-world found`) in
## main.tscn, or without `--open` draws and founds a terrarium through the picker. Checks:
## the pause menu's Dev metrics opens the panel; it lists every lineage the `vitals` query
## returns at the newest minute, with a meaning on every column header; the Plants and
## Charts views fill; back (the gamepad's B) closes it to the menu; F3 opens it over the
## running world and Esc closes it; and no error is logged while it runs. Prints PASS/FAIL
## lines, exits 0 only if all passed.
##
## Pictures (needs a renderer: run it on a private display, never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/metrics_test.gd -- --open SAVE --shots DIR [--minutes N]
## runs the world at top speed until N minutes of rows are held (default 6), then writes
## DIR/metrics-table.png and DIR/metrics-chart.png.

const TIMEOUT_S := 240.0


class Errors extends Logger:
	var lines := []

	func _log_error(function: String, file: String, line: int, code: String, rationale: String,
			_editor_notify: bool, error_type: int, _script_backtraces: Array) -> void:
		if error_type == ERROR_TYPE_WARNING:
			return
		lines.append("%s:%d %s %s %s" % [file, line, function, code, rationale])


var main
var failures := 0
var started := 0
var saves_dir := ""
var shots := ""
var want_minutes := 6
var errors := Errors.new()


func check(ok: bool, what: String) -> void:
	print(("PASS " if ok else "FAIL ") + what)
	if not ok:
		failures += 1


func _initialize() -> void:
	started = Time.get_ticks_msec()
	OS.add_logger(errors)
	saves_dir = OS.get_user_data_dir().path_join("metrics-test-saves")
	var open := ""
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--saves":
				saves_dir = args[i + 1]
			"--open":
				open = args[i + 1]
			"--shots":
				shots = args[i + 1]
			"--minutes":
				want_minutes = int(args[i + 1])
	_wipe(saves_dir)
	if open != "":
		_copy(open, saves_dir.path_join("Metrics"))
	run(open != "")


func run(opened: bool) -> void:
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves_dir
	main.settings_path = saves_dir + ".cfg"
	main.autoload = opened
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	if not opened:
		main.picker.select("terrarium")
		main.picker._seed.text = "5"
		main.picker.generate(1)
		if await until(func(): return main.picker.ready_count() == 1, 120.0):
			main.picker._cards[0]["pick"].pressed.emit()
	var running := await until(func(): return main.has_world() and main.busy() == "", 200.0)
	check(running, "a world runs (%d s)" % elapsed())
	if not running:
		return finish()
	await until(func(): return not main.overlay_open(), 10.0)

	# From the pause menu.
	esc()
	await frames(2)
	check(main.menu.visible and not main.menu.buttons["metrics"].disabled, "the menu offers Dev metrics")
	main.menu.buttons["metrics"].pressed.emit()
	await frames(2)
	var panel = main.metrics
	check(panel.visible and not main.menu.visible, "Dev metrics opens the panel in the menu's place")
	var q = main.session.query("vitals", null)
	check(q is Dictionary and q.has("animals") and q["animals"].size() > 0, "the session serves vitals rows")
	var held := await until(func(): return not panel.minutes.is_empty(), 10.0)
	check(held, "the panel holds rows")
	q = main.session.query("vitals", null)
	var lineage_at: int = q["animal_columns"].map(func(c): return c["name"]).find("lineage")
	var newest := -1.0
	for r in q["animals"]:
		newest = maxf(newest, float(r[0]))
	var served := []
	for r in q["animals"]:
		if float(r[0]) == newest:
			served.append(r[lineage_at])
	await until(func(): return panel.shown_minute() >= newest, 5.0)
	panel.show_view("animals")
	await frames(1)
	check(panel.shown_minute() == newest and panel.listed_lineages() == served,
		"the table lists every lineage the query returns at minute %s: %s" % [newest, served])
	check(q["animal_columns"].all(func(c): return str(c["doc"]) != "") and q["plant_columns"].all(func(c): return str(c["doc"]) != ""),
		"every column comes with its meaning")
	var tips := _labels(panel._animal_grid).filter(func(l): return l.tooltip_text != "")
	check(tips.size() >= panel.shown_groups().reduce(func(n, g): return n + g[1].size(), 0),
		"every column header has a tooltip (%d)" % tips.size())
	panel.show_view("plants")
	await frames(1)
	check(_labels(panel._plant_grid).size() > panel.plant_columns.size(), "the plants table fills")
	panel.show_view("charts")
	await frames(1)
	check(panel._all.series.size() == panel.lineages().size() and panel._one.series.size() == 1,
		"the charts hold one population series per lineage and the picked column")
	panel.show_view("animals")

	# Back closes it to the menu.
	pad(JOY_BUTTON_B)
	await frames(2)
	check(not panel.visible and main.menu.visible, "back closes it to the menu")
	esc()
	await until(func(): return not main.session.status()["paused"], 5.0)
	check(not main.menu.visible and not main.session.status()["paused"], "and Esc resumes the world")

	# F3 over the running world, Esc closes it.
	key(KEY_F3)
	await frames(2)
	check(panel.visible and not main.menu.visible, "F3 opens the panel over the world")
	check(not main.session.status()["paused"], "the world keeps running under it")
	if shots != "":
		await pictures(panel)
	esc()
	await frames(2)
	check(not panel.visible and not main.menu.visible, "Esc closes it, without the menu")
	key(KEY_F3)
	await frames(1)
	key(KEY_F3)
	await frames(1)
	check(not panel.visible, "F3 again closes it")
	check(errors.lines.is_empty(), "no errors logged%s" % ("" if errors.lines.is_empty() else ": " + "\n  ".join(errors.lines.slice(0, 8))))
	finish()


func pictures(panel) -> void:
	DirAccess.make_dir_recursive_absolute(shots)
	main.session.console("speed 64")
	var ok := await until(func(): return panel.minutes.size() >= want_minutes, TIMEOUT_S - elapsed() - 20)
	print("     %d minutes held for the pictures" % panel.minutes.size())
	main.session.console("speed 1")
	panel.show_view("animals")
	await frames(3)
	await shot("metrics-table")
	panel.show_view("charts")
	var names: Array = panel.lineages()
	for k in panel._lineage_pick.item_count:
		if panel._lineage_pick.get_item_metadata(k) == "frondgrazer":
			panel._lineage_pick.select(k)
	for k in panel._column_pick.item_count:
		if panel._column_pick.get_item_text(k) == "fullness_p50_adult":
			panel._column_pick.select(k)
	panel._redraw_charts()
	await frames(3)
	await shot("metrics-chart")
	check(ok, "rows of %d minutes for the pictures (%s lineages)" % [want_minutes, names.size()])


func shot(name: String) -> void:
	await RenderingServer.frame_post_draw
	var img := root.get_texture().get_image()
	var path := shots.path_join(name + ".png")
	img.save_png(path)
	print("     wrote " + path)


func finish() -> void:
	print("METRICS %s (%d failures, %d s)" % ["OK" if failures == 0 else "FAILED", failures, elapsed()])
	_wipe(saves_dir)
	DirAccess.remove_absolute(saves_dir + ".cfg")
	main.queue_free()
	await frames(2)
	quit(0 if failures == 0 else 1)


# --- Helpers ------------------------------------------------------------------------

func elapsed() -> int:
	return (Time.get_ticks_msec() - started) / 1000


func frames(n: int) -> void:
	for i in n:
		await process_frame


func until(cond: Callable, timeout_s: float) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not cond.call():
		if Time.get_ticks_msec() > deadline or elapsed() > TIMEOUT_S:
			return false
		await process_frame
	return true


func key(k: Key) -> void:
	for down in [true, false]:
		var e := InputEventKey.new()
		e.physical_keycode = k
		e.keycode = k
		e.pressed = down
		root.push_input(e)


func esc() -> void:
	key(KEY_ESCAPE)


func pad(b: JoyButton) -> void:
	for down in [true, false]:
		var e := InputEventJoypadButton.new()
		e.button_index = b
		e.pressed = down
		root.push_input(e)


func _labels(node: Node) -> Array:
	var out := []
	for c in node.get_children():
		if c is Label:
			out.append(c)
	return out


func _copy(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))


func _wipe(path: String) -> void:
	preload("res://ui/saves.gd").remove_tree(path)
