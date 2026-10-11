extends Node
## **The app shell** (package C of design/handoffs/godot-direction-2026-10-07.md): the live
## world through a `WorldView`, the pause menu, save slots, the new-world picker, the
## tabbed page (Field guide / Populations / Settings) and the animal inspect panel. Rust is the simulation (`CubariumSession`); this file owns the
## flow between screens and the one reader of the session's notices.
##
## Start-up opens the title menu. Continue opens the latest attached autosave; --new opens the picker.
## Esc: closes a dialog, then the open screen, then the inspect panel, then toggles the
## pause menu. Esc never quits; only the menu's Quit does. Gamepad: Start toggles the
## menu, B goes back, Select (or P) pauses the world without a menu. F3 opens the dev
## metrics panel over the running world (the pause menu's Dev metrics opens it too); back,
## Esc or F3 again closes it. G or the pad's Y opens the tabbed page on the Field guide (the
## pause menu's Field guide and Settings open it on that tab); back or Esc closes it, to
## the pause menu when it came from there; inside, LB / RB (or Q / E) switch tabs and Y
## is Show in world. Only the pause menu pauses the world: from it the tabbed page
## resumes the world (unless the player paused it), and back to the menu pauses it again.
##
## Command line (after `--`): `--saves DIR` (the slots' root, default `user://saves`),
## `--no-autoload` (accepted for older launchers; startup always waits), `--new` (open the picker at start-up).

const WindowFit := preload("res://ui/window_fit.gd")
const WorldViews := preload("res://ui/world_views.gd")
const Saves := preload("res://ui/saves.gd")
const PauseMenu := preload("res://ui/pause_menu.tscn")
const SaveSlots := preload("res://ui/save_slots.tscn")
const NewWorld := preload("res://ui/new_world.tscn")
const InspectPanel := preload("res://ui/inspect_panel.tscn")
const MetricsPanel := preload("res://ui/dev/metrics_panel.gd")
const TabbedPage := preload("res://ui/tabbed_page/tabbed_page.tscn")
const Dialog := preload("res://ui/dialog.tscn")
const Notices := preload("res://ui/notices.tscn")
const PausedBadge := preload("res://ui/paused_badge.tscn")
const BusyPanel := preload("res://ui/busy_panel.tscn")
const THEME := preload("res://ui/theme.tres")
const SETTINGS := "user://shell.cfg"
const THUMB := Vector2i(320, 160)

## Set before the node enters the tree (tests, pictures), or from the command line.
var saves_root := ""
var autoload := true
var start_new := false
## Where the shell keeps its own small settings (last slot, founding times, the Settings
## tab's choices).
var settings_path := SETTINGS
## What Quit does; tests swap it for a recorder.
var quit_hook: Callable

var session
var world_view: Node
var saves
var settings := ConfigFile.new()
var species := {}  # lineage code name -> label

var ui: Control
var menu
var slots
var picker
var metrics
## The metrics panel was opened from the pause menu, which it goes back to.
var metrics_from_menu := false
## The tabbed page, and its Field guide and Settings tabs' contents.
var tabbed
var guide
var settings_page
## The tabbed page was opened from the pause menu, which it goes back to.
var tabbed_from_menu := false
var inspect
var dialog
var notices
var tooltip: PanelContainer
var tooltip_label: Label
var busy_panel: PanelContainer
var busy_label: Label
var paused_label: Control

## The player paused the world (Select or P) outside any menu.
var player_paused := false
var pending_save := {}
var pending_load := ""
var pending_autosave := ""
var queued_load := ""
var pending_thumbnail := {}
var thumbnail_handoff := {}
var quit_after_save := false
var quit_save_done := false
var _hovered := -1


func _ready() -> void:
	_read_args()
	_actions()
	get_tree().auto_accept_quit = false
	if not quit_hook.is_valid():
		quit_hook = func(): get_tree().quit()
	settings.load(settings_path)
	session = CubariumSession.new()
	var table = session.query("species", null)
	if table is Dictionary:
		for a in table.get("animals", []):
			species[a["name"]] = a["label"]
	if saves_root == "":
		saves_root = ProjectSettings.globalize_path("user://saves")
	saves = Saves.new(saves_root)

	world_view = WorldViews.make()
	add_child(world_view)
	if world_view is Control:
		(world_view as Control).set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	world_view.attach(session)
	world_view.animal_hovered.connect(_on_hovered)
	world_view.animal_clicked.connect(_on_clicked)
	_apply_stored_motion()
	_build_ui()
	if world_view.has_method("set_scope_enabled"):
		world_view.set_scope_enabled(settings.get_value("view", "zoom_scope", true))

	open_menu()
	if start_new:
		open_picker()


func _read_args() -> void:
	var args := OS.get_cmdline_user_args()
	for i in args.size():
		match args[i]:
			"--saves":
				if i + 1 < args.size():
					saves_root = args[i + 1]
			"--no-autoload":
				autoload = false
			"--new":
				start_new = true


## The shell's input actions, made here so the project file stays package A's.
func _actions() -> void:
	var key := func(k: Key) -> InputEvent:
		var e := InputEventKey.new()
		e.physical_keycode = k
		return e
	var pad := func(b: JoyButton) -> InputEvent:
		var e := InputEventJoypadButton.new()
		e.button_index = b
		return e
	for a in [["shell_menu", [key.call(KEY_ESCAPE), pad.call(JOY_BUTTON_START)]],
			["shell_back", [pad.call(JOY_BUTTON_B)]],
			["shell_pause", [key.call(KEY_P), pad.call(JOY_BUTTON_BACK)]],
			["shell_metrics", [key.call(KEY_F3)]],
			["shell_guide", [key.call(KEY_G), pad.call(JOY_BUTTON_Y)]],
			["shell_tab_prev", [key.call(KEY_Q), pad.call(JOY_BUTTON_LEFT_SHOULDER)]],
			["shell_tab_next", [key.call(KEY_E), pad.call(JOY_BUTTON_RIGHT_SHOULDER)]]]:
		if InputMap.has_action(a[0]):
			continue
		InputMap.add_action(a[0])
		for e in a[1]:
			InputMap.action_add_event(a[0], e)


func _build_ui() -> void:
	var layer := CanvasLayer.new()
	layer.layer = 10
	add_child(layer)
	ui = Control.new()
	ui.theme = THEME
	ui.set_anchors_and_offsets_preset(Control.PRESET_TOP_LEFT)
	ui.mouse_filter = Control.MOUSE_FILTER_IGNORE
	layer.add_child(ui)

	# The neon screens (ui/*.tscn) carry ui/neon/neon_theme.tres themselves; the picker,
	# Settings and the dev metrics keep the dev theme above.
	paused_label = PausedBadge.instantiate()
	paused_label.visible = false
	ui.add_child(paused_label)

	inspect = InspectPanel.instantiate()
	inspect.shell = self
	inspect.closed.connect(_on_inspect_closed)
	ui.add_child(inspect)

	busy_panel = BusyPanel.instantiate()
	busy_panel.visible = false
	busy_label = busy_panel.get_node("Margin/Text")
	ui.add_child(busy_panel)

	menu = PauseMenu.instantiate()
	menu.shell = self
	menu.chosen.connect(_on_menu)
	ui.add_child(menu)

	slots = SaveSlots.instantiate()
	slots.shell = self
	slots.back.connect(func(): close_screen(slots))
	ui.add_child(slots)

	picker = NewWorld.instantiate()
	picker.shell = self
	picker.back.connect(func(): close_screen(picker))
	ui.add_child(picker)
	picker.setup()

	metrics = MetricsPanel.new()
	metrics.shell = self
	metrics.back.connect(close_metrics)
	ui.add_child(metrics)

	tabbed = TabbedPage.instantiate()
	tabbed.shell = self
	ui.add_child(tabbed)
	guide = tabbed.guide
	guide.show_in_world.connect(_on_show_in_world)
	settings_page = tabbed.settings
	settings_page.changed.connect(_on_motion_changed)
	settings_page.scope_changed.connect(_on_scope_changed)

	dialog = Dialog.instantiate()
	dialog.shell = self
	ui.add_child(dialog)

	# On top of everything: the hover tooltip and the notices.
	tooltip = PanelContainer.new()
	tooltip.theme_type_variation = &"Tip"
	tooltip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	tooltip.visible = false
	tooltip_label = Label.new()
	tooltip.add_child(tooltip_label)
	ui.add_child(tooltip)

	notices = Notices.instantiate()
	notices.shell = self
	ui.add_child(notices)
	get_viewport().size_changed.connect(_fit_ui)
	_fit_ui()


func _fit_ui() -> void:
	WindowFit.apply(ui, get_viewport().get_visible_rect().size)
	WindowFit.fit_inspector(inspect, ui)


# --- State --------------------------------------------------------------------------

func has_world() -> bool:
	return session.status().get("running", false)


## "saving", "loading" or "".
func busy() -> String:
	if pending_autosave != "":
		return "saving"
	if not pending_save.is_empty():
		return "saving"
	if pending_load != "":
		return "loading"
	var status: Dictionary = session.status()
	if status.get("saving", false):
		return "saving"
	return ""


## Whether anything covers the world: the menu, a screen or a dialog.
func overlay_open() -> bool:
	return menu.visible or slots.visible or picker.visible or tabbed.visible or metrics.visible or dialog.is_open()


func species_label(name: String) -> String:
	return species.get(name, name)


func _last_slot() -> String:
	var slots_now: Array = saves.list(session)
	var named: String = settings.get_value("shell", "last_slot", "")
	var home: String = saves.home_of(named) if named != "" else ""
	# Continue resumes the attached autosave even when a newer manual snapshot exists.
	for s in slots_now:
		if s["name"] == home and s.get("loadable", false):
			return s["name"]
	for s in slots_now:
		if saves.is_autosave(s["name"]) and s.get("loadable", false):
			return s["name"]
	for s in slots_now:
		if s.get("loadable", false):
			return s["name"]
	return ""


func founding_estimate_s(key: String) -> float:
	return settings.get_value("founding_s", key, 0.0)


func record_founding_s(key: String, seconds: float) -> void:
	settings.set_value("founding_s", key, seconds)
	settings.save(settings_path)


func _remember_slot(name: String) -> void:
	settings.set_value("shell", "last_slot", name)
	settings.save(settings_path)


# --- Input --------------------------------------------------------------------------

func _input(event: InputEvent) -> void:
	if event.is_echo():
		return
	if event.is_action_pressed("shell_menu"):
		get_viewport().set_input_as_handled()
		escape(event is InputEventJoypadButton)
	elif event.is_action_pressed("shell_back"):
		if back_one():
			get_viewport().set_input_as_handled()
	elif event.is_action_pressed("shell_metrics") and not _typing():
		get_viewport().set_input_as_handled()
		if metrics.visible:
			close_metrics()
		elif has_world() and not dialog.is_open() and not slots.visible and not picker.visible and not tabbed.visible:
			open_metrics()
	elif event.is_action_pressed("shell_guide") and not _typing():
		if guide.visible:
			# Inside the guide the pad's Y is Show in world; G closes it.
			get_viewport().set_input_as_handled()
			if event is InputEventJoypadButton:
				guide.press_show()
			else:
				close_tabbed()
		elif tabbed.visible:
			# On another tab, G or Y goes to the Field guide tab.
			get_viewport().set_input_as_handled()
			if not dialog.is_open():
				tabbed.show_tab("guide")
		elif has_world() and not dialog.is_open() and not slots.visible and not picker.visible and not metrics.visible:
			get_viewport().set_input_as_handled()
			open_guide()
	elif (event.is_action_pressed("shell_tab_prev") or event.is_action_pressed("shell_tab_next")) and tabbed.visible:
		# Q and E typed into the search box stay text; LB and RB switch tabs from anywhere.
		if not dialog.is_open() and not (_typing() and event is InputEventKey):
			get_viewport().set_input_as_handled()
			tabbed.step(-1 if event.is_action_pressed("shell_tab_prev") else 1)
	elif event.is_action_pressed("shell_pause") and not overlay_open() and not _typing():
		get_viewport().set_input_as_handled()
		set_player_paused(not player_paused)


func _typing() -> bool:
	var focus := get_viewport().gui_get_focus_owner()
	return focus is LineEdit or focus is TextEdit


## Esc (or Start): close the innermost thing open; with nothing open, toggle the menu.
## Start skips the inspect panel and goes straight to the menu.
func escape(start := false) -> void:
	if back_one(start):
		return
	if menu.visible:
		close_menu()
	else:
		open_menu()


## Close the innermost dialog, screen or panel; false when only the menu (or nothing)
## is open. Closing the menu itself counts, when a world can resume.
func back_one(start := false) -> bool:
	if dialog.is_open():
		dialog.cancel()
		return true
	if picker.visible:
		if not picker.back_one():
			close_screen(picker)
		return true
	if slots.visible:
		close_screen(slots)
		return true
	if metrics.visible:
		close_metrics()
		return true
	if tabbed.visible:
		close_tabbed()
		return true
	if inspect.visible and not menu.visible and not start:
		inspect.close()
		return true
	if menu.visible and not start and has_world():
		close_menu()
		return true
	return false


func set_player_paused(on: bool) -> void:
	player_paused = on
	if on:
		session.pause()
	else:
		session.resume()


# --- Menu and screens ---------------------------------------------------------------

func open_menu() -> void:
	session.pause()
	menu.open(has_world(), _last_slot() if not has_world() else "")
	world_view.set("input_enabled", false)
	_hover_off()


func close_menu() -> void:
	if not has_world():
		menu.focus_first()
		return
	menu.visible = false
	if not player_paused:
		session.resume()


func _on_menu(item: String) -> void:
	match item:
		"resume":
			if has_world():
				close_menu()
			else:
				var last := _last_slot()
				if last != "":
					load_slot(last)
		"save":
			slots.open("save")
		"load":
			slots.open("load")
		"new":
			open_picker()
		"settings":
			open_settings()
		"metrics":
			open_metrics()
		"guide":
			open_guide()
		"quit":
			request_quit()


func open_picker() -> void:
	if has_world() and busy() == "":
		_request_autosave("switch")
	picker.open()


## Back from a screen to the menu under it. Leaving the picker cancels what it was
## drawing or founding.
func close_screen(screen: Control) -> void:
	if screen == picker:
		picker.leave()
	screen.visible = false
	if not menu.visible:
		open_menu()
	else:
		menu.focus_first()


func close_all() -> void:
	dialog.cancel()
	picker.leave()
	picker.visible = false
	slots.visible = false
	metrics.visible = false
	tabbed.close()
	menu.visible = false


func _process(_delta: float) -> void:
	for n in session.take_notices():
		_on_notice(n)
	for n in session.take_save_notices():
		_on_autosave_notice(n)
	_refresh_autosave_thumbnail()
	if quit_after_save and busy() == "":
		if quit_save_done or not has_world():
			quit_after_save = false
			quit_hook.call()
		else:
			_request_autosave("quit")
	world_view.set("input_enabled", not overlay_open() and busy() != "loading" and not _typing())
	paused_label.visible = player_paused and not overlay_open()
	busy_panel.visible = (pending_load != "" or pending_autosave in ["initial", "attach", "quit"]) and not slots.visible
	if tooltip.visible:
		var at := ui.get_local_mouse_position() + Vector2(18, 18)
		at.x = clampf(at.x, 8.0, maxf(8.0, ui.size.x - tooltip.size.x - 8))
		at.y = clampf(at.y, 8.0, maxf(8.0, ui.size.y - tooltip.size.y - 8))
		tooltip.position = at
	if inspect.visible and inspect.dead:
		world_view.stop_follow()


# --- Dev metrics --------------------------------------------------------------------

## The dev metrics panel over the world. From the pause menu it stands in for the menu
## (the world stays paused) and goes back to it; from F3 the world keeps running under it.
func open_metrics() -> void:
	metrics_from_menu = menu.visible
	menu.visible = false
	_hover_off()
	metrics.open()


func close_metrics() -> void:
	metrics.visible = false
	if metrics_from_menu:
		open_menu()


# --- The tabbed page: Field guide, Populations, Settings ------------------------------

## The tabbed page over the world on tab `tab`. From the pause menu it stands in for the
## menu and goes back to it; the world runs under it (unless the player paused it with
## Select or P), and the menu pauses it again on the way back. From G or Y the pause state
## doesn't change.
func open_tabbed(tab: String) -> void:
	tabbed_from_menu = menu.visible
	menu.visible = false
	_hover_off()
	if tabbed_from_menu and has_world() and not player_paused:
		session.resume()
	tabbed.open(tab, has_world())


func close_tabbed() -> void:
	tabbed.close()
	if tabbed_from_menu:
		open_menu()


## The field guide: the tabbed page on its Field guide tab.
func open_guide() -> void:
	open_tabbed("guide")


func close_guide() -> void:
	close_tabbed()


## Show in world: close the guide (and the menu under it), pan to one living animal of
## `lineage` (an adult when there is one) and select it as a click does.
func _on_show_in_world(lineage: String) -> void:
	var rows = session.query("animals", null)
	var pick := -1
	if rows is Array:
		for a in rows:
			if a.get("species", "") != lineage:
				continue
			if pick < 0 or str(a.get("stage", "")) == "adult":
				pick = int(a.get("id", -1))
				if str(a.get("stage", "")) == "adult":
					break
	if pick < 0:
		notices.add("No living %s to show." % species_label(lineage), true)
		return
	tabbed.close()
	tabbed_from_menu = false
	_on_clicked(pick)


# --- Settings -----------------------------------------------------------------------

## The Settings tab's choices: in the shell's settings file, per viewer. A viewer who has
## chosen them has them applied as the renderer starts, after the environment's overrides
## (what `CUBARIUM_ANIMALS` or `CUBARIUM_ANIMAL_SUBPIXEL` names keeps the environment's
## value for this run); a change on the page applies to the running world at once, over
## both. Without a stored choice the config and the environment decide.
func stored_motion() -> Dictionary:
	if not settings.has_section_key("animals", "movement"):
		return {}
	return {"movement": settings.get_value("animals", "movement", "smooth"),
		"subpixel": settings.get_value("animals", "subpixel", true)}


func _apply_stored_motion() -> void:
	var m := stored_motion()
	if not m.is_empty():
		world_view.set_animal_motion(m["movement"], m["subpixel"], true)


## The settings in force: what the renderer draws with once it runs, else the stored ones.
func current_motion() -> Dictionary:
	var live: Dictionary = world_view.animal_motion() if world_view.has_method("animal_motion") else {}
	if live.has("movement"):
		return {"movement": live["movement"], "subpixel": live["subpixel"]}
	var m := stored_motion()
	return m if not m.is_empty() else {"movement": "smooth", "subpixel": true}


## The tabbed page on its Settings tab, which shows the settings in force.
func open_settings() -> void:
	open_tabbed("settings")


func _on_scope_changed(enabled: bool) -> void:
	settings.set_value("view", "zoom_scope", enabled)
	settings.save(settings_path)
	if world_view.has_method("set_scope_enabled"):
		world_view.set_scope_enabled(enabled)


func _on_motion_changed(movement: String, subpixel: bool) -> void:
	settings.set_value("animals", "movement", movement)
	settings.set_value("animals", "subpixel", subpixel)
	settings.save(settings_path)
	world_view.set_animal_motion(movement, subpixel)


# --- Saves --------------------------------------------------------------------------

## Save the running world into slot `name` (new or overwriting): "" once asked, or why
## not. The outcome arrives as a notice.
func save_slot(name: String) -> String:
	if busy() != "":
		return "Wait for the %s to finish." % busy()
	if not has_world():
		return "No world is running."
	var problem: String = saves.name_problem(name)
	if problem != "":
		return problem
	if saves.is_autosave(name):
		return "This name belongs to a world autosave. Choose a snapshot name."
	# `--headless` renders nothing: no thumbnail rather than a blank one.
	var thumb: Image = null
	if world_view.has_method("capture") and DisplayServer.get_name() != "headless":
		thumb = _current_thumbnail()
	var dir: String = saves.begin(name)
	pending_save = {"name": name, "dir": dir, "thumb": thumb, "home": String(session.home()).get_file()}
	session.save(dir)
	slots.say("Saving \"%s\"…" % name)
	return ""


func load_slot(name: String) -> void:
	if busy() != "":
		notices.add("Wait for the %s to finish." % busy(), true)
		return
	if has_world():
		queued_load = name
		_request_autosave("load")
		return
	_load_now(name)


func _load_now(name: String) -> void:
	thumbnail_handoff = {"session": session.status()["generation"], "frame": world_view.get("frame_generation")}
	pending_load = name
	busy_label.text = "Opening \"%s\"…" % name
	if slots.visible:
		slots.say("Loading \"%s\"…" % name)
	session.load_with_home(saves.path_of(name), saves.path_of(saves.home_of(name)))


func renamed_slot(from: String, to: String) -> void:
	if String(session.home()).get_file() == from:
		session.attach_home(saves.path_of(to))
	if settings.get_value("shell", "last_slot", "") == from:
		_remember_slot(to)
	notices.add("Renamed \"%s\" to \"%s\"." % [from, to])


func deleting_active_world(name: String) -> bool:
	return has_world() and String(session.home()).get_file() == name


func deleted_slot(name: String) -> void:
	if deleting_active_world(name):
		session.close()
		inspect.close()
		close_all()
		# The close command takes effect on the sim thread; title mode is immediate.
		menu.open(false, _last_slot())
	if settings.get_value("shell", "last_slot", "") == name:
		_remember_slot("")
	notices.add("Deleted \"%s\"." % name)


func _request_autosave(reason: String) -> void:
	var problem: String = session.autosave()
	if problem != "":
		quit_after_save = false
		queued_load = ""
		notices.add("Autosave failed: " + problem, true)
		return
	pending_autosave = reason
	busy_label.text = "Saving the world before quitting…" if reason == "quit" else "Saving the world…"


func _on_autosave_notice(n: Dictionary) -> void:
	var requested: bool = n.get("requested", false)
	var reason := pending_autosave if requested else ""
	if requested:
		pending_autosave = ""
	if n.get("kind", "") == "autosave_failed":
		quit_after_save = false
		quit_save_done = false
		queued_load = ""
		notices.add("Autosave failed: %s" % n.get("text", ""), true)
		slots.say("Autosave failed: %s" % n.get("text", ""))
		if reason == "quit":
			open_menu()
		return
	if DisplayServer.get_name() != "headless" and n.get("path", "") == String(session.home()):
		pending_thumbnail = {"path": n["path"], "generation": session.status()["generation"]}
	if reason == "load" and not quit_after_save:
		var name := queued_load
		queued_load = ""
		_load_now(name)
	if reason == "quit":
		quit_save_done = true
	if reason == "initial":
		notices.add("A new world. Autosave ready.")
	if slots.visible:
		slots.refresh()


## Never capture the previous world's frame while its replacement is still rendering.
func _current_thumbnail() -> Image:
	var generation = world_view.get("frame_generation")
	if generation == null or int(generation) < 0 or not world_view.get("lighting_ready"):
		return null
	# Hosted has its own generation counter; compare it to the frame before the handoff,
	# while the session counter tells us whether that handoff has installed a world yet.
	if not thumbnail_handoff.is_empty() and session.status()["generation"] > thumbnail_handoff["session"]:
		if generation == thumbnail_handoff["frame"]:
			return null
		thumbnail_handoff = {}
	return world_view.capture(THUMB)


## A thumbnail follows a completed save once its current world is visible; saving and
## quitting keep progressing independently of the renderer.
func _refresh_autosave_thumbnail() -> void:
	if pending_thumbnail.is_empty():
		return
	var path: String = pending_thumbnail["path"]
	if String(session.home()) != path or session.status().get("generation", -1) != pending_thumbnail["generation"] or not DirAccess.dir_exists_absolute(path):
		pending_thumbnail = {}
		return
	if busy() != "":
		return
	var thumb := _current_thumbnail()
	if thumb == null:
		return
	pending_thumbnail = {}
	var temporary := path.path_join(Saves.THUMB + ".next")
	var err := thumb.save_png(temporary)
	if err == OK:
		err = DirAccess.rename_absolute(temporary, path.path_join(Saves.THUMB))
	if err != OK:
		DirAccess.remove_absolute(temporary)
		notices.add("Thumbnail could not be saved: " + error_string(err), true)
	elif slots.visible:
		slots.refresh()


func request_quit() -> void:
	if quit_after_save:
		return
	session.cancel()
	session.pause()
	quit_after_save = true
	quit_save_done = false


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		request_quit()


func _on_notice(n: Dictionary) -> void:
	var kind: String = n.get("kind", "")
	match kind:
		"saved":
			if pending_save.is_empty() or n.get("path", "") != pending_save["dir"]:
				return
			var name: String = pending_save["name"]
			var problem: String = saves.commit(name, pending_save["thumb"], pending_save["home"])
			pending_save = {}
			if problem == "":
				notices.add("Saved \"%s\"." % name)
				slots.say("Saved \"%s\"." % name)
			else:
				notices.add("Save failed: " + problem, true)
				slots.say("Save failed: " + problem)
			if slots.visible:
				slots.refresh()
		"save_failed":
			if pending_save.is_empty() or n.get("path", "") != pending_save["dir"]:
				return
			var name: String = pending_save.get("name", "")
			if not pending_save.is_empty():
				Saves.remove_tree(pending_save["dir"])
			pending_save = {}
			notices.add("Save failed: %s" % n.get("text", ""), true)
			slots.say("Save of \"%s\" failed: %s" % [name, n.get("text", "")])
			if slots.visible:
				slots.refresh()
		"loaded":
			var name := pending_load
			pending_load = ""
			if name != "":
				_remember_slot(saves.home_of(name))
			notices.add("Loaded \"%s\"." % name if name != "" else "Loaded.")
			inspect.close()
			close_all()
			world_view.reset_framing()
			set_player_paused(false)
			if not DirAccess.dir_exists_absolute(String(session.home())):
				_request_autosave("attach")
		"load_failed":
			var name := pending_load
			pending_load = ""
			var text := "Couldn't load \"%s\": %s" % [name, n.get("text", "")]
			notices.add(text, true)
			if slots.visible:
				slots.say(text)
				slots.refresh()
			elif not has_world() and not picker.visible:
				open_menu()


# --- New worlds ---------------------------------------------------------------------

## Found candidate `i` on its own terrain: "" once it has started, or why not. The
## picker follows it (`session.founding()`) and calls `on_founded` when it runs.
func found_candidate(i: int) -> String:
	if busy() != "":
		return "Wait for the %s to finish." % busy()
	var before := {"session": session.status()["generation"], "frame": world_view.get("frame_generation")}
	var problem: String = session.promote_into(i, saves.path_of(saves.new_home()))
	if problem == "":
		thumbnail_handoff = before
	return problem


## The picked world is founded and installed: close everything over it and run it.
func on_founded(_founding: Dictionary) -> void:
	inspect.close()
	close_all()
	world_view.reset_framing()
	set_player_paused(false)
	_remember_slot(String(session.home()).get_file())
	_request_autosave("initial")


# --- Inspect ------------------------------------------------------------------------

func _on_hovered(id: int) -> void:
	_hovered = id
	if id < 0 or overlay_open():
		_hover_off()
		return
	var q = session.query("animal", {"id": id})
	var body = q.get("animal") if q is Dictionary else null
	if not (body is Dictionary):
		_hover_off()
		return
	tooltip_label.text = "%s  ·  %s" % [body.get("label", "?"), body.get("stage", "")]
	tooltip.reset_size()
	tooltip.visible = true


func _hover_off() -> void:
	tooltip.visible = false


func _on_clicked(id: int) -> void:
	if overlay_open():
		return
	if id < 0:
		inspect.close()
		return
	inspect.open(id)
	world_view.set_highlight(id)
	world_view.follow(id)


func _on_inspect_closed() -> void:
	world_view.set_highlight(-1)
	world_view.stop_follow()
