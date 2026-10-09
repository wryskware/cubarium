extends SceneTree
## Headless test of the app shell's mechanisms (never tuned numbers):
##   godot --headless --path game --script res://tests/shell_test.gd [-- --saves DIR]
## Runs main.tscn against a scratch saves directory and checks: start-up with no save opens
## the picker; Esc never quits; two candidates of the smallest size found and one is
## promoted; Esc opens the menu and pauses, Esc again resumes, and a player's own pause
## survives the menu; Start opens the menu; the inspect panel opens on a living animal and
## Esc closes it before the menu opens; save, list, rename, delete through the slots
## screen; a save from another version is listed, refused on load, and can be deleted;
## a good save loads back; the Settings page (from the menu) reloads the viewer's stored
## animal settings into the renderer at start-up, its two toggles reach the running
## world's renderer at once (the frames drawn report them) and are stored again, and
## Subpixel is off under Stepped. Prints PASS/FAIL lines, exits 0 only if all passed.

const TIMEOUT_S := 250.0
const WorldViews := preload("res://ui/world_views.gd")
const NewWorld := preload("res://ui/new_world.gd")

var main
var failures := 0
var quits := 0
var started := 0
var saves_dir := ""


func check(ok: bool, what: String) -> void:
	print(("PASS " if ok else "FAIL ") + what)
	if not ok:
		failures += 1


func _initialize() -> void:
	started = Time.get_ticks_msec()
	saves_dir = OS.get_user_data_dir().path_join("shell-test-saves")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i] == "--saves":
			saves_dir = args[i + 1]
	_wipe(saves_dir)
	_wipe(saves_dir + ".staging")
	run()


func run() -> void:
	# A viewer's stored animal settings, which start-up must load into the renderer.
	var stored := ConfigFile.new()
	stored.set_value("animals", "movement", "stepped")
	stored.set_value("animals", "subpixel", false)
	stored.save(saves_dir + ".cfg")
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves_dir
	main.settings_path = saves_dir + ".cfg"
	main.quit_hook = func(): quits += 1
	root.add_child(main)
	await frames(3)

	# Start-up without a save: the picker, over the menu.
	check(main.picker.visible, "start-up with no save opens the new-world picker")
	check(not main.has_world(), "no world runs yet")

	# Esc never quits; with no world it closes the picker to the menu and the menu stays.
	esc()
	await frames(2)
	check(not main.picker.visible and main.menu.visible, "Esc closes the picker back to the menu")
	for k in 4:
		esc()
		await frames(1)
	check(quits == 0, "Esc never quits (5 presses, no quit)")
	check(main.menu.visible, "with no world, Esc leaves the menu up (nothing to resume)")

	# Terrain candidates of the smallest size: bare terrains, nothing founded on them.
	main._on_menu("new")
	await frames(1)
	check(main.picker.visible, "New world opens the picker")
	main.picker.select("terrarium")
	var info = main.session.query("spec", main.picker.spec(false))
	var start_step := 1
	for k in main.picker.water_levels.size():
		if info is Dictionary and info.get("water_level_m") != null \
				and absf(float(main.picker.water_levels[k]["water_level_m"]) - float(info["water_level_m"])) < 1e-6:
			start_step = k
	check(main.picker._water_box.visible and main.picker._water.index == start_step,
		"the water slider starts on the config's own table (%s m: step %d)" % [str(info.get("water_level_m")), start_step])
	check(NewWorld.seed_problem("18446744073709551615") == "" and NewWorld.seed_problem("18446744073709551616") != "",
		"a seed may be any unsigned 64-bit number")
	main.picker._seed.text = "5"  # a seed whose terrain founds on its first try
	var refused: String = main.picker.generate(2)
	check(refused == "", "two terrarium terrains asked for %s" % refused)
	var ready := await until(func(): return main.picker.ready_count() == 2, 90.0)
	check(ready, "both terrains drawn and shown (%d s)" % elapsed())
	if not ready:
		await finish()
		return
	var cands: Array = main.session.candidates()
	check(cands.all(func(c): return c["state"] == "ready" and str(c.get("terrain", "")) != "" and c.get("feasible") != null),
		"each candidate carries its terrain hash and the feasibility check's verdict")
	check(_living(cands) == 0, "nothing is founded on a terrain candidate")
	check(cands[0]["seed"] is String and cands[0]["seed"] == "5" and cands[1]["seed"] == "6", "seeds cross the binding as strings: %s" % [cands.map(func(c): return c["seed"])])
	check(not main.has_world(), "nothing runs while picking")
	var card: Dictionary = main.picker._cards[1]
	check(card["holder"].get_child_count() > 1, "a ready candidate shows its own WorldView")
	main.picker.open_large(1)
	await frames(1)
	check(main.picker.is_large_open() and main.picker.large_view() != null, "a candidate opens large")
	print("     the world is drawn by %s" % ("package B's WorldView" if WorldViews.is_real() else "the shell's stub"))
	esc()
	await frames(1)
	check(not main.picker.is_large_open() and main.picker.visible, "Esc closes the large candidate, the picker stays")

	# The water slider brings the same terrains back at another level.
	var seeds: Array = cands.map(func(c): return c["seed"])
	var other := 2 if start_step != 2 else 0
	main.picker._water.choose(other)
	await frames(1)
	check(is_equal_approx(float(main.picker.spec().get("water_level_m", -1.0)), float(main.picker.water_levels[other]["water_level_m"])),
		"the slider sets the spec's water_level_m (%s)" % str(main.picker.spec().get("water_level_m")))
	ready = await until(func(): return main.picker.ready_count() == 2, 90.0)
	check(ready and main.session.candidates().map(func(c): return c["seed"]) == seeds,
		"moving the slider draws the same terrains again: %s" % [main.session.candidates().map(func(c): return c["seed"])])
	main.picker._water.choose(start_step)
	ready = await until(func(): return main.picker.ready_count() == 2, 90.0)
	check(ready, "and back at the config's own level")

	# Drawing can be cancelled.
	main.picker._seed.text = ""
	main.picker.generate(3)
	await frames(1)
	check(main.picker._cancel.visible, "Cancel shows while terrains are drawn")
	main.picker._cancel.pressed.emit()
	await frames(1)
	check(main.session.candidates().all(func(c): return c["state"] != "generating"), "Cancel stops the draws: %s" % [main.session.candidates().map(func(c): return c["state"])])
	var stopped := await until(func(): return main.session.workers() == 0, 60.0)
	check(stopped, "and their threads end")

	# Back to the two terrains, then a founding that is cancelled.
	main.picker._seed.text = "5"
	main.picker.generate(2)
	ready = await until(func(): return main.picker.ready_count() == 2, 90.0)
	check(ready, "the two terrains again")
	var terrain0: String = str(main.session.candidates()[0]["terrain"])
	main.picker._cards[1]["root"].pressed.emit()
	await frames(2)
	check(main.picker.is_founding() and main.picker._found_box.visible and main.picker._generate.disabled,
		"picking one shows the founding panel and holds the rest")
	await until(func(): return str(main.session.founding().get("phase", "")) != "", 30.0)
	main.picker._found_cancel.pressed.emit()
	await until(func(): return not main.picker.is_founding(), 10.0)
	check(main.session.founding()["state"] == "cancelled" and not main.has_world() and main.session.candidates().size() == 2,
		"cancelling the founding installs nothing and keeps the terrains")
	stopped = await until(func(): return main.session.workers() == 0, 60.0)
	check(stopped, "the founding thread ends")

	# The picked terrain founded, with a real progress bar, and run.
	main.picker._cards[0]["root"].pressed.emit()
	var progress := []
	var phases := []
	var bar := []
	var founded := await until(func():
		var now: Dictionary = main.session.founding()
		if now["state"] == "founding":
			var p := float(now.get("progress", 0.0))
			if progress.is_empty() or p != progress[-1]:
				progress.append(p)
			if not str(now.get("phase", "")) in phases:
				phases.append(str(now.get("phase", "")))
			bar.append(main.picker._found_bar.value)
		return now["state"] != "founding", 200.0)
	var fd: Dictionary = main.session.founding()
	check(founded and fd["state"] == "done", "candidate 0 founded (%d s): %s" % [int(fd.get("elapsed_s", 0)), str(fd.get("error", ""))])
	check(progress.size() > 3 and progress.any(func(p): return p > 0.0 and p < 1.0), "the bar moved with the founding: %d readings, %s" % [progress.size(), phases])
	check(bar.any(func(v): return v > 0.0 and v < 1.0), "the picker's bar showed it")
	check(str(fd.get("terrain", "")) == terrain0, "the founded world kept the picked terrain")
	await until(func(): return main.has_world(), 20.0)
	check(main.has_world(), "picking candidate 0 runs it")
	check(not main.picker.visible and not main.menu.visible, "picking closes the picker and the menu")
	check(main.session.candidates().size() == 0, "the other candidate is dropped")
	await until(func(): return not main.session.status()["paused"], 5.0)
	check(not main.session.status()["paused"], "the new world runs unpaused")

	# Esc pauses with the menu, Esc again resumes.
	esc()
	await until(func(): return main.session.status()["paused"], 5.0)
	await frames(1)
	check(main.menu.visible and main.session.status()["paused"], "Esc opens the menu and pauses")
	check(not main.world_view.input_enabled, "the world takes no input under the menu")
	esc()
	await until(func(): return not main.session.status()["paused"], 5.0)
	check(not main.menu.visible and not main.session.status()["paused"], "Esc again closes the menu and resumes")
	await frames(1)  # the shell sets the view's input on its next frame
	check(main.world_view.input_enabled, "the world takes input again")
	# A pause of the player's own survives the menu.
	key(KEY_P)
	await until(func(): return main.session.status()["paused"], 5.0)
	esc()
	await frames(2)
	esc()
	await frames(5)
	check(main.player_paused and main.session.status()["paused"], "closing the menu keeps the player's own pause")
	key(KEY_P)
	await until(func(): return not main.session.status()["paused"], 5.0)
	check(not main.session.status()["paused"], "P resumes")
	# Start opens the menu too.
	pad(JOY_BUTTON_START)
	await frames(1)
	check(main.menu.visible, "the gamepad's Start opens the menu")
	pad(JOY_BUTTON_B)
	await frames(1)
	check(not main.menu.visible, "the gamepad's B closes it")
	check(quits == 0, "still no quit")

	# Inspect a living animal.
	await until(func(): return _first_animal() >= 0, 10.0)
	var id := _first_animal()
	check(id >= 0, "a living animal to inspect (#%d)" % id)
	main.world_view.animal_hovered.emit(id)
	await frames(1)
	check(main.tooltip.visible and main.tooltip_label.text != "", "hovering shows a tooltip: " + main.tooltip_label.text)
	main.world_view.animal_hovered.emit(-1)
	main.world_view.animal_clicked.emit(id)
	await frames(2)
	check(main.inspect.visible and main.inspect.animal_id == id, "clicking opens the inspect panel")
	var body = main.inspect.last.get("animal")
	check(body is Dictionary and body.get("id") == id and main.inspect._stats.get_child_count() > 0, "the panel shows the animal's stats from query(\"animal\")")
	check(main.world_view.highlighted == id and main.world_view.following == id, "the panel highlights and follows it")
	esc()
	await frames(1)
	check(not main.inspect.visible and not main.menu.visible, "Esc closes the panel first, without opening the menu")
	check(main.world_view.highlighted == -1 and main.world_view.following == -1, "closing it clears the highlight and the follow")

	await settings_checks()

	# Save two slots through the slots screen.
	esc()
	await frames(1)
	main._on_menu("save")
	await frames(1)
	check(main.slots.visible, "Save opens the slots")
	for name in ["Alpha", "Beta"]:
		main.slots._name.text = name
		main.slots._on_save_new()
		var saved := await until(func(): return main.busy() == "" and main.saves.exists(name), 30.0)
		check(saved, "saved \"%s\" (%s)" % [name, main.slots._status.text])
	var listed: Array = main.saves.list(main.session)
	var names := listed.map(func(s): return s["name"])
	check("Alpha" in names and "Beta" in names, "both slots listed: %s" % [names])
	var alpha = listed.filter(func(s): return s["name"] == "Alpha")
	check(alpha.size() == 1 and alpha[0]["meta"] != null and alpha[0]["meta"]["day"] >= 1 and alpha[0]["loadable"], "a slot has its day and species from meta.json")
	# `--headless` renders nothing, so only the stub can draw a thumbnail here; the pictures
	# (tests/shots.gd) show the real view's.
	if main.world_view.get_script() == WorldViews.STUB or DisplayServer.get_name() != "headless":
		check(alpha.size() == 1 and alpha[0]["thumb"] != "", "a slot has its thumbnail")
	check(main.notices.said.any(func(t): return t.begins_with("Saved")), "the save outcome is shown as a notice")
	# Overwrite asks first.
	main.slots._overwrite("Beta")
	check(main.dialog.is_open(), "overwrite asks for confirmation")
	esc()
	await frames(1)
	check(not main.dialog.is_open() and main.slots.visible, "Esc cancels the question and keeps the slots")
	# Rename through the dialog.
	main.slots._rename("Alpha")
	main.dialog._field.text = "Beta"
	main.dialog._confirm()
	check(main.dialog.is_open() and main.saves.exists("Alpha"), "renaming onto a taken name is refused")
	main.dialog._field.text = "Gamma"
	main.dialog._confirm()
	await frames(1)
	check(not main.dialog.is_open() and main.saves.exists("Gamma") and not main.saves.exists("Alpha"), "renamed Alpha to Gamma")
	# Delete through the dialog.
	main.slots._delete("Beta")
	main.dialog._confirm()
	await frames(1)
	check(not main.saves.exists("Beta"), "deleted Beta")
	names = main.saves.list(main.session).map(func(s): return s["name"])
	check(names == ["Gamma"], "the list is now %s" % [names])

	# A save from another version: listed, refused, deletable.
	_copy_tree(main.saves.path_of("Gamma"), main.saves.path_of("Old"))
	var meta_path: String = main.saves.path_of("Old").path_join("meta.json")
	# Edit the text, not a re-serialized copy: Godot's JSON would write the integers back
	# as floats, which the session reads as no metadata at all.
	var text := FileAccess.get_file_as_string(meta_path)
	var schema := RegEx.create_from_string("\"world\":\\s*(\\d+)").search(text)
	check(schema != null, "the copy's meta.json names its world schema")
	if schema != null:
		var bumped := "\"world\": %d" % (int(schema.get_string(1)) + 1000)
		text = text.substr(0, schema.get_start()) + bumped + text.substr(schema.get_end())
	var f := FileAccess.open(meta_path, FileAccess.WRITE)
	f.store_string(text)
	f.close()
	main.slots.refresh()
	var old = main.saves.list(main.session).filter(func(s): return s["name"] == "Old")
	check(old.size() == 1 and not old[0]["loadable"], "an older-version save is listed, not loadable")
	var row = main.slots._rows.get("Old")
	check(row != null and not row["load"].visible and not row["delete"].disabled, "its row offers Delete, not Load")
	check(row != null and _row_text(row["row"]).contains("older version"), "its row says it was made by an older version")
	main.load_slot("Old")
	await until(func(): return main.busy() == "", 30.0)
	check(main.notices.said.any(func(t): return t.begins_with("Couldn't load \"Old\"")), "loading it is refused with a notice: %s" % [main.notices.said.slice(-2)])
	check(main.has_world(), "the running world carries on after the refusal")
	main.slots._delete("Old")
	main.dialog._confirm()
	await frames(1)
	check(not main.saves.exists("Old"), "the older-version save can be deleted")

	# A good save loads back.
	var gen: int = main.session.status()["generation"]
	main.load_slot("Gamma")
	var loaded := await until(func(): return main.busy() == "" and main.session.status()["generation"] > gen, 60.0)
	check(loaded and main.has_world(), "Gamma loads")
	check(not main.slots.visible and not main.menu.visible, "a load closes the menus")
	check(quits == 0, "no quit through the whole run")
	main._on_menu("quit")
	check(quits == 1, "the menu's Quit quits")
	finish()


## The Settings page on the running world: start-up loaded the stored settings, each toggle
## reaches the renderer's next frames, and both are stored for the next start.
func settings_checks() -> void:
	var real: bool = main.world_view.get_script() != WorldViews.STUB
	var at_start: Dictionary = main.world_view.animal_motion()
	var env: bool = at_start.get("env_movement", false) or at_start.get("env_subpixel", false)
	if env:
		print("     CUBARIUM_ANIMALS or CUBARIUM_ANIMAL_SUBPIXEL is set: start-up keeps the environment's values")
	else:
		check(at_start.get("movement") == "stepped" and at_start.get("subpixel") == false,
			"start-up loads the stored animal settings into the renderer: %s" % [at_start])
	if real:
		check(await until(func(): return main.world_view.animal_motion().get("drawn_movement") == at_start.get("movement"), 10.0),
			"the running world is drawn with them: %s" % [main.world_view.animal_motion()])
	esc()
	await frames(1)
	check(main.menu.visible and main.menu.buttons.has("settings") and not main.menu.buttons["settings"].disabled,
		"the menu offers Settings")
	main.menu.buttons["settings"].pressed.emit()
	await frames(1)
	var page = main.settings_page
	check(page.visible, "Settings opens the page")
	check(page.movement == at_start.get("movement") and page.subpixel == at_start.get("subpixel"),
		"the page shows the settings in force (%s, subpixel %s)" % [page.movement, page.subpixel])
	check(not page.subpixel_enabled() if page.movement == "stepped" else page.subpixel_enabled(),
		"Subpixel is off under Stepped and on under Smooth")
	# Movement: Smooth. Reaches the renderer at once and is stored.
	page.choose_movement("smooth")
	await frames(1)
	check(page.subpixel_enabled(), "under Smooth, Subpixel can be changed")
	if real:
		check(await drawn_is("drawn_movement", "smooth"), "Smooth reaches the running renderer: %s" % [main.world_view.animal_motion()])
	else:
		check(main.world_view.animal_motion().get("movement") == "smooth", "Smooth reaches the view")
	# Subpixel: the other way. Reaches the renderer and is stored.
	var want: bool = not page.subpixel
	page.choose_subpixel(want)
	await frames(1)
	if real:
		check(await drawn_is("drawn_subpixel", want), "Subpixel %s reaches the running renderer: %s" % [want, main.world_view.animal_motion()])
	else:
		check(main.world_view.animal_motion().get("subpixel") == want, "Subpixel reaches the view")
	var reread := ConfigFile.new()
	reread.load(main.settings_path)
	check(reread.get_value("animals", "movement", "") == "smooth" and reread.get_value("animals", "subpixel", null) == want,
		"both are stored for the next start: %s, %s" % [reread.get_value("animals", "movement", ""), reread.get_value("animals", "subpixel", null)])
	# And Stepped again, while it runs: Subpixel greys out, the renderer steps.
	page.choose_movement("stepped")
	await frames(1)
	check(not page.subpixel_enabled(), "under Stepped, Subpixel is off")
	if real:
		check(await drawn_is("drawn_movement", "stepped"), "Stepped reaches the running renderer")
	esc()
	await frames(1)
	check(not page.visible and main.menu.visible, "Esc closes Settings back to the menu")
	esc()
	await until(func(): return not main.session.status()["paused"], 5.0)


## Whether the renderer's frames come to be drawn with `key` = `value` within 10 s.
func drawn_is(key: String, value) -> bool:
	return await until(func(): return main.world_view.animal_motion().get(key) == value, 10.0)


func finish() -> void:
	print("SHELL %s (%d failures, %d s)" % ["OK" if failures == 0 else "FAILED", failures, elapsed()])
	_wipe(saves_dir)
	_wipe(saves_dir + ".staging")
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


## Wait until `cond` holds, up to `timeout_s`; whether it did.
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


## Animals, eggs and plant stands over every candidate's populations.
func _living(cands: Array) -> int:
	var n := 0
	for c in cands:
		var pops = c.get("populations")
		if not (pops is Dictionary):
			continue
		for a in pops.get("animals", []):
			n += int(a.get("alive", 0)) + int(a.get("eggs", 0))
		for p in pops.get("plants", []):
			n += int(p.get("stands", 0))
	return n


func _first_animal() -> int:
	var animals = main.session.query("animals", null)
	if animals is Array and animals.size() > 0:
		return animals[0]["id"]
	return -1


func _row_text(node: Node) -> String:
	var out := ""
	if node is Label:
		out += node.text + "\n"
	for c in node.get_children():
		out += _row_text(c)
	return out


func _copy_tree(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))


func _wipe(path: String) -> void:
	preload("res://ui/saves.gd").remove_tree(path)
