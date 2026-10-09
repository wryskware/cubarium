extends SceneTree
## The field guide in the running app (design/handoffs/field-guide-live-2026-10-09.md):
## mechanisms only, never tuned numbers.
##   godot --headless --path game --script res://tests/guide_test.gd -- --open SAVE
## Opens the save (a slot directory, e.g. from `cubarium-world found`) in main.tscn and
## checks: the pause menu's Field guide opens it (the tabbed page on Field guide), the world
## runs under it, and back (the pad's B) returns to the menu, which pauses again; the
## menu's Settings opens the tabbed page on Settings, LB/RB, Q/E and a click switch tabs,
## and Esc returns to the menu; G opens it over the running world on Field guide without
## changing the pause state (nor does E, to Settings; G there goes back to Field guide),
## and Esc closes it; the pad's Y opens it and B closes
## it; the list holds every species in game/data/field_guide.md with the populations
## query's counts; search filters by name; the facts come from the `guide` query; Show in
## world selects a living animal of the species; no error is logged. Prints PASS/FAIL lines,
## exits 0 only if all passed.
##
## Pictures (needs a renderer: run it on a private display, never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/guide_test.gd -- --open SAVE --shots DIR [--run-s N]
## runs the world at speed 64 for N wall seconds first (default 45) so the bars have some
## history, then writes DIR/guide-*.png.

class Errors extends Logger:
	var lines := []

	func _log_error(function: String, file: String, line: int, code: String, rationale: String,
			_editor_notify: bool, error_type: int, _script_backtraces: Array) -> void:
		if error_type == ERROR_TYPE_WARNING:
			return
		lines.append("%s:%d %s %s %s" % [file, line, function, code, rationale])


var main
var guide
var failures := 0
var shots := ""
var run_s := 45.0
var errors := Errors.new()


func check(ok: bool, what: String) -> void:
	print(("PASS " if ok else "FAIL ") + what)
	if not ok:
		failures += 1


func _initialize() -> void:
	OS.add_logger(errors)
	var args := OS.get_cmdline_user_args()
	var save := ""
	for i in args.size() - 1:
		match args[i]:
			"--open": save = args[i + 1]
			"--shots": shots = args[i + 1]
			"--run-s": run_s = float(args[i + 1])
	if save == "":
		print("FAIL usage: -- --open SAVE [--shots DIR]")
		quit(2)
		return
	run(save)


func run(save: String) -> void:
	var saves := OS.get_user_data_dir().path_join("guide-test-saves")
	preload("res://ui/saves.gd").remove_tree(saves)
	_copy(save, saves.path_join("World"))
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	guide = main.guide
	check(await until(func(): return main.has_world() and main.busy() == "", 120.0), "the save opens")
	await seconds(1.0)

	# The pause menu's item opens it in place of the menu; back goes back to the menu.
	main.escape()
	await frames(2)
	check(main.menu.buttons.has("guide") and not main.menu.buttons["guide"].disabled, "the pause menu has Field guide")
	main.menu.buttons["guide"].pressed.emit()
	await frames(3)
	check(guide.visible and not main.menu.visible, "Field guide opens from the menu")
	check(main.tabbed.visible and main.tabbed.tab == "guide", "the menu's Field guide opens the tabbed page on Field guide")
	# Only the pause menu pauses: the world runs under the tabbed page, and back to the menu
	# pauses it again (Wrysk, 2026-10-09: "field guide doesnt pause the world").
	check(await until(func(): return not main.session.status().get("paused", true), 5.0), "the world runs under it")
	await key(pad(JOY_BUTTON_B))
	check(not guide.visible and main.menu.visible, "back closes it to the menu")
	check(await until(func(): return main.session.status().get("paused", false), 5.0), "back to the menu pauses the world again")

	# The menu's Settings opens it on Settings; LB/RB and Q/E switch tabs; back goes to the menu.
	main.menu.buttons["settings"].pressed.emit()
	await frames(3)
	check(main.tabbed.visible and main.tabbed.tab == "settings" and main.settings_page.visible and not guide.visible,
		"the menu's Settings opens the tabbed page on Settings")
	check(await until(func(): return not main.session.status().get("paused", true), 5.0), "the world runs under the Settings tab")
	await key(pad(JOY_BUTTON_LEFT_SHOULDER))
	check(main.tabbed.tab == "guide" and guide.visible and not main.settings_page.visible, "LB switches to the Field guide tab (Populations is skipped, it's off)")
	await key(pad(JOY_BUTTON_LEFT_SHOULDER))
	check(main.tabbed.tab == "guide", "LB at the first tab stays there")
	await key(pad(JOY_BUTTON_RIGHT_SHOULDER))
	check(main.tabbed.tab == "settings" and main.settings_page.visible and not guide.visible, "RB switches to the Settings tab")
	await key(keyev(KEY_Q))
	check(main.tabbed.tab == "guide" and guide.visible, "Q switches to the tab on the left")
	await key(keyev(KEY_E))
	check(main.tabbed.tab == "settings" and main.settings_page.visible, "E switches to the tab on the right")
	main.tabbed.buttons["guide"].pressed.emit()
	await frames(2)
	check(main.tabbed.tab == "guide" and guide.visible, "a click on a tab switches to it")
	await key(keyev(KEY_ESCAPE))
	check(not main.tabbed.visible and main.menu.visible, "Esc closes it back to the menu")
	check(await until(func(): return main.session.status().get("paused", false), 5.0), "and the menu pauses the world again")
	main.close_menu()
	await frames(2)

	# G over the running world, Esc closes; the pad's Y, B closes. Neither changes pause.
	var was_paused: bool = main.session.status().get("paused", false)
	await key(keyev(KEY_G))
	check(guide.visible and not main.menu.visible, "G opens it over the world")
	check(main.tabbed.visible and main.tabbed.tab == "guide", "G opens the tabbed page on Field guide")
	check(main.session.status().get("paused", false) == was_paused, "opening it leaves the pause state")
	await key(keyev(KEY_E))
	check(main.tabbed.tab == "settings" and main.session.status().get("paused", false) == was_paused,
		"switching tabs leaves the pause state")
	await key(keyev(KEY_G))
	check(main.tabbed.tab == "guide" and guide.visible, "G on another tab goes to Field guide")
	await key(keyev(KEY_ESCAPE))
	check(not guide.visible and not main.menu.visible, "Esc closes it back to the world")
	await key(pad(JOY_BUTTON_Y))
	check(guide.visible, "the pad's Y opens it")
	await key(pad(JOY_BUTTON_B))
	check(not guide.visible and not main.menu.visible, "the pad's B closes it back to the world")

	if shots != "":
		main.session.set_speed(64.0)
		await seconds(run_s)
		main.session.set_speed(1.0)

	main.open_guide()
	await seconds(1.5)
	print("     history samples held: %d; springturf bars %s; vaulttree bars %s" % [guide._samples.size(), guide.bar_values("springturf"), guide.bar_values("vaulttree")])
	# The list: every notes entry once, with the populations query's counts.
	var ids := []
	for n in guide.notes:
		ids.append(n.get("id", ""))
	check(guide.notes.size() == 19 and ids.has("latticevine") and ids.has("chorister"), "the notes list 9 animals and 10 plants (%d)" % guide.notes.size())
	var pops = main.session.query("populations", null)
	var counts_ok := true
	for a in pops.get("animals", []):
		var r = guide._rows.get(a["species"])
		var want: int = int(a["adults"]) + int(a["juveniles"]) + int(a["pups"]) + int(a["riders"])
		if r == null or abs(int(r.count) - want) > max(2, want / 10):
			counts_ok = false
			print("     %s row %s, populations %d" % [a["species"], r.count if r != null else "missing", want])
	check(counts_ok, "each animal row shows adults plus juveniles from populations")
	var table = main.session.query("guide", null)
	var lj: Dictionary = {}
	for e in table.get("animals", []):
		if e["name"] == "lanternjaw":
			lj = e
	guide.select("lanternjaw")
	await frames(2)
	var body: String = guide._facts.get_node("BodyCell/Value").text
	check(body == "%s m long · %s m tall" % [guide.metres(lj["length_m"]), guide.metres(lj["height_m"])], "Body comes from the guide query (%s)" % body)
	check(guide._chips.get_child_count() == lj["prey"].size() + lj["diet"].size(), "Eats has one chip per prey lineage and food class")
	check(guide._caption.text.begins_with("Plate 01"), "the plate caption numbers by list order (%s)" % guide._caption.text)
	if shots != "":
		var r = guide._rows.get("lanternjaw")
		r.grab_focus()
		await shot("guide-lanternjaw")
		guide._entry_scroll.scroll_vertical = 100000
		await shot("guide-lanternjaw-eats")
		guide._entry_scroll.scroll_vertical = 0
		guide.select("vaulttree")
		guide._set_plants_open(true)
		await frames(2)
		guide._rows["vaulttree"].grab_focus()
		await shot("guide-vaulttree")
		guide.select("latticevine")
		await frames(2)
		guide._rows["latticevine"].grab_focus()
		await shot("guide-latticevine")
		# The list with Plants open, scrolled partway: focus walks down to a row near the end.
		guide.select("lanternjaw")
		guide._rows["lanternjaw"].grab_focus()
		await frames(2)
		for i in 15:
			await key(keyev(KEY_DOWN))
		await shot("guide-list-plants-open")
	# Search filters by name.
	guide._search.grab_focus()
	guide._search.text = "glow"
	guide._search.text_changed.emit("glow")
	await frames(2)
	var shown := []
	for id in guide._rows:
		if guide._rows[id].visible:
			shown.append(id)
	check(shown == ["glowcap"], "search 'glow' leaves glowcap only (%s)" % [shown])
	await key(keyev(KEY_G))
	check(guide.visible, "G typed in the search box doesn't close it (search text '%s')" % guide._search.text)
	guide._search.text = "lan"
	guide._search.text_changed.emit("lan")
	await frames(2)
	if shots != "":
		await shot("guide-search")
	guide._search.text = ""
	guide._search.text_changed.emit("")
	guide._set_plants_open(false)
	# A long field note scrolls above the fixed buttons.
	if shots != "":
		guide.select("loftstrider")
		var note: Label = guide._text.get_node("FieldNote")
		note.text = " ".join([note.text, note.text, note.text])
		await frames(3)
		guide._entry_scroll.scroll_vertical = 100000
		guide._show.grab_focus()
		await frames(3)
		await shot("guide-long-note")
		var actions: Control = guide._entry.get_node("TextColumn/Actions")
		check(guide._entry_scroll.scroll_vertical > 0 and actions.get_global_rect().end.y <= guide._entry.get_global_rect().end.y + 1,
				"a long note scrolls; the buttons stay below it")

	# Show in world: an animal of the species, selected the way a click does.
	var living := ""
	for a in main.session.query("populations", null).get("animals", []):
		if int(a["adults"]) + int(a["juveniles"]) > 0 and living == "":
			living = a["species"]
	guide.select(living)
	await frames(2)
	if not guide._show.disabled:
		guide.press_show()
		await seconds(1.0)
		var q = main.session.query("animal", {"id": main.inspect.animal_id})
		var species: String = q.get("animal", {}).get("species", "") if q is Dictionary and q.get("animal") is Dictionary else ""
		check(not guide.visible and main.inspect.visible and species == living, "Show in world selects a living %s (%s)" % [living, species])
		if shots != "":
			await seconds(1.5)
			await shot("guide-show-in-world")
	else:
		check(false, "Show in world is enabled for a living %s" % living)

	check(errors.lines.is_empty(), "no errors logged")
	for l in errors.lines:
		print("     ", l)
	print("GUIDE TEST %s (%d failed)" % ["PASS" if failures == 0 else "FAIL", failures])
	preload("res://ui/saves.gd").remove_tree(saves)
	quit(0 if failures == 0 else 1)


func keyev(k: Key) -> InputEvent:
	var e := InputEventKey.new()
	e.physical_keycode = k
	e.keycode = k
	e.unicode = 103 if k == KEY_G else 0
	e.pressed = true
	return e


func pad(b: JoyButton) -> InputEvent:
	var e := InputEventJoypadButton.new()
	e.button_index = b
	e.pressed = true
	return e


func key(e: InputEvent) -> void:
	Input.parse_input_event(e)
	await frames(2)
	var up = e.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	await frames(2)


func shot(name: String) -> void:
	await frames(3)
	await RenderingServer.frame_post_draw
	var img := root.get_texture().get_image()
	DirAccess.make_dir_recursive_absolute(shots)
	img.save_png(shots.path_join(name + ".png"))
	print("SHOT ", shots.path_join(name + ".png"))


func frames(n: int) -> void:
	for i in n:
		await process_frame


func seconds(s: float) -> void:
	await create_timer(s).timeout


func until(cond: Callable, timeout_s: float) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not cond.call():
		if Time.get_ticks_msec() > deadline:
			return false
		await process_frame
	return true


func _copy(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))
