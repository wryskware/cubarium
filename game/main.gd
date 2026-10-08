extends Node
## **The app shell** (package C of design/handoffs/godot-direction-2026-10-07.md): the live
## world through a `WorldView`, the pause menu, save slots, the new-world picker and the
## animal inspect panel. Rust is the simulation (`CubariumSession`); this file owns the
## flow between screens and the one reader of the session's notices.
##
## Start-up opens the last save if there is one, else the new-world picker.
## Esc: closes a dialog, then the open screen, then the inspect panel, then toggles the
## pause menu. Esc never quits; only the menu's Quit does. Gamepad: Start toggles the
## menu, B goes back, Select (or P) pauses the world without a menu.
##
## Command line (after `--`): `--saves DIR` (the slots' root, default `user://saves`),
## `--no-autoload` (don't open the last save), `--new` (open the picker at start-up).

const WorldViews := preload("res://ui/world_views.gd")
const Saves := preload("res://ui/saves.gd")
const PauseMenu := preload("res://ui/pause_menu.gd")
const SaveSlots := preload("res://ui/save_slots.gd")
const NewWorld := preload("res://ui/new_world.gd")
const InspectPanel := preload("res://ui/inspect_panel.gd")
const Dialog := preload("res://ui/dialog.gd")
const Notices := preload("res://ui/notices.gd")
const THEME := preload("res://ui/theme.tres")
const SETTINGS := "user://shell.cfg"
const THUMB := Vector2i(320, 160)

## Set before the node enters the tree (tests, pictures), or from the command line.
var saves_root := ""
var autoload := true
var start_new := false
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
var inspect
var dialog
var notices
var tooltip: PanelContainer
var tooltip_label: Label
var busy_panel: PanelContainer
var busy_label: Label
var paused_label: Label

## The player paused the world (Select or P) outside any menu.
var player_paused := false
var pending_save := {}
var pending_load := ""
var _hovered := -1


func _ready() -> void:
	_read_args()
	_actions()
	if not quit_hook.is_valid():
		quit_hook = func(): get_tree().quit()
	settings.load(SETTINGS)
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
	_build_ui()

	var last := _last_slot() if autoload and not start_new else ""
	if last != "":
		load_slot(last)
	else:
		open_menu()
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
			["shell_pause", [key.call(KEY_P), pad.call(JOY_BUTTON_BACK)]]]:
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
	ui.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	ui.mouse_filter = Control.MOUSE_FILTER_IGNORE
	layer.add_child(ui)

	paused_label = Label.new()
	paused_label.text = "Paused"
	paused_label.position = Vector2(16, 12)
	paused_label.visible = false
	ui.add_child(paused_label)

	inspect = InspectPanel.new()
	inspect.shell = self
	inspect.closed.connect(_on_inspect_closed)
	ui.add_child(inspect)

	tooltip = PanelContainer.new()
	tooltip.theme_type_variation = &"Tip"
	tooltip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	tooltip.visible = false
	tooltip_label = Label.new()
	tooltip.add_child(tooltip_label)
	ui.add_child(tooltip)

	notices = Notices.new()
	ui.add_child(notices)

	busy_panel = PanelContainer.new()
	busy_panel.theme_type_variation = &"Tip"
	busy_panel.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	busy_panel.grow_horizontal = Control.GROW_DIRECTION_BOTH
	busy_panel.grow_vertical = Control.GROW_DIRECTION_BOTH
	busy_panel.visible = false
	busy_label = Label.new()
	busy_label.add_theme_font_size_override("font_size", 22)
	busy_panel.add_child(busy_label)
	ui.add_child(busy_panel)

	menu = PauseMenu.new()
	menu.chosen.connect(_on_menu)
	ui.add_child(menu)

	slots = SaveSlots.new()
	slots.shell = self
	slots.back.connect(func(): close_screen(slots))
	ui.add_child(slots)

	picker = NewWorld.new()
	picker.shell = self
	picker.back.connect(func(): close_screen(picker))
	ui.add_child(picker)
	picker.setup()

	dialog = Dialog.new()
	ui.add_child(dialog)


# --- State --------------------------------------------------------------------------

func has_world() -> bool:
	return session.status().get("running", false)


## "saving", "loading" or "".
func busy() -> String:
	if not pending_save.is_empty():
		return "saving"
	if pending_load != "":
		return "loading"
	return ""


## Whether anything covers the world: the menu, a screen or a dialog.
func overlay_open() -> bool:
	return menu.visible or slots.visible or picker.visible or dialog.is_open()


func species_label(name: String) -> String:
	return species.get(name, name)


func _last_slot() -> String:
	var slots_now: Array = saves.list(session)
	var named: String = settings.get_value("shell", "last_slot", "")
	for s in slots_now:
		if s["name"] == named and s.get("loadable", false):
			return named
	for s in slots_now:
		if s.get("loadable", false):
			return s["name"]
	return ""


func founding_estimate_s(key: String) -> float:
	return settings.get_value("founding_s", key, 0.0)


func record_founding_s(key: String, seconds: float) -> void:
	settings.set_value("founding_s", key, seconds)
	settings.save(SETTINGS)


func _remember_slot(name: String) -> void:
	settings.set_value("shell", "last_slot", name)
	settings.save(SETTINGS)


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
	elif event.is_action_pressed("shell_pause") and not overlay_open() and not _typing():
		get_viewport().set_input_as_handled()
		set_player_paused(not player_paused)


func _typing() -> bool:
	return get_viewport().gui_get_focus_owner() is LineEdit


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
	menu.open(has_world())
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
			close_menu()
		"save":
			slots.open("save")
		"load":
			slots.open("load")
		"new":
			open_picker()
		"quit":
			quit_hook.call()


func open_picker() -> void:
	picker.open()


## Back from a screen to the menu under it.
func close_screen(screen: Control) -> void:
	screen.visible = false
	if not menu.visible:
		open_menu()
	else:
		menu.focus_first()


func close_all() -> void:
	dialog.cancel()
	picker.visible = false
	slots.visible = false
	menu.visible = false


func _process(_delta: float) -> void:
	for n in session.take_notices():
		_on_notice(n)
	world_view.set("input_enabled", not overlay_open() and busy() != "loading")
	paused_label.visible = player_paused and not overlay_open()
	busy_panel.visible = pending_load != "" and not slots.visible
	if tooltip.visible:
		var at := ui.get_local_mouse_position() + Vector2(18, 18)
		at.x = minf(at.x, ui.size.x - tooltip.size.x - 8)
		at.y = minf(at.y, ui.size.y - tooltip.size.y - 8)
		tooltip.position = at
	if inspect.visible and inspect.dead:
		world_view.stop_follow()


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
	var thumb: Image = world_view.capture(THUMB) if world_view.has_method("capture") else null
	var dir: String = saves.begin(name)
	pending_save = {"name": name, "dir": dir, "thumb": thumb}
	session.save(dir)
	slots.say("Saving \"%s\"…" % name)
	return ""


func load_slot(name: String) -> void:
	if busy() != "":
		notices.add("Wait for the %s to finish." % busy(), true)
		return
	pending_load = name
	busy_label.text = "Opening \"%s\"…" % name
	if slots.visible:
		slots.say("Loading \"%s\"…" % name)
	session.load(saves.path_of(name))


func renamed_slot(from: String, to: String) -> void:
	if settings.get_value("shell", "last_slot", "") == from:
		_remember_slot(to)
	notices.add("Renamed \"%s\" to \"%s\"." % [from, to])


func deleted_slot(name: String) -> void:
	notices.add("Deleted \"%s\"." % name)


func _on_notice(n: Dictionary) -> void:
	var kind: String = n.get("kind", "")
	match kind:
		"saved":
			if pending_save.is_empty():
				return
			var name: String = pending_save["name"]
			var problem: String = saves.commit(name, pending_save["thumb"])
			pending_save = {}
			if problem == "":
				_remember_slot(name)
				notices.add("Saved \"%s\"." % name)
				slots.say("Saved \"%s\"." % name)
			else:
				notices.add("Save failed: " + problem, true)
				slots.say("Save failed: " + problem)
			if slots.visible:
				slots.refresh()
		"save_failed":
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
				_remember_slot(name)
			notices.add("Loaded \"%s\"." % name if name != "" else "Loaded.")
			inspect.close()
			close_all()
			world_view.reset_framing()
			set_player_paused(false)
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
				open_picker()


# --- New worlds ---------------------------------------------------------------------

## Run candidate `i`: "" or why not.
func promote_candidate(i: int) -> String:
	var problem: String = session.promote(i)
	if problem != "":
		return problem
	inspect.close()
	close_all()
	world_view.reset_framing()
	set_player_paused(false)
	notices.add("A new world.")
	return ""


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
	inspect.open(id)
	world_view.set_highlight(id)
	world_view.follow(id)


func _on_inspect_closed() -> void:
	world_view.set_highlight(-1)
	world_view.stop_follow()
