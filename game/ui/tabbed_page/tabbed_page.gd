extends Control
## **The tabbed page** (design/handoffs/neon-menu-tabs-2026-10-09.md, the hybrid): Field
## guide / Populations / Settings in one neon frame. The look is tabbed_page.tscn, laid out
## in the editor: the tab strip and the clock on top, the main panel, the key hints. Each
## tab's content is its own scene instanced into the panel (ui/field_guide/field_guide.tscn,
## ui/settings.tscn); Populations isn't designed yet and stays disabled.
##
## The shell (main.gd) opens it on a tab (the pause menu's Field guide and Settings, or G /
## the pad's Y for the field guide) and closes it with back or Esc. LB / RB (Q / E) step
## between the enabled tabs (the shell's input), and a click or A on a tab shows it. Only
## the shown tab's content is visible, and only while the page is open. The page never
## pauses or resumes the world itself; the clock's Paused shows the session's real state.
## Without a shell (the editor, or a picture of the scene) it keeps the sample page.

## Tab ids in strip order.
const TABS := ["guide", "populations", "settings"]
const REFRESH_S := 1.0

## The shell (main.gd); null in the editor or a picture, which keeps the sample page.
var shell: Node
## The tab shown ("guide" or "settings").
var tab := "guide"

var _refresh_left := 0.0

@onready var _world_picture: Control = $World
@onready var _clock: Control = $Center/Page/Top/Clock
@onready var _strip: Control = $Center/Page/Top/Tabs/TabStrip
@onready var _hints: Control = $Center/Page/Hints
@onready var guide: Control = $Center/Page/Body/FieldGuide
@onready var settings: Control = $Center/Page/Body/Settings
@onready var buttons := {
	"guide": _strip.get_node("FieldguideTab"),
	"populations": _strip.get_node("PopulationsTab"),
	"settings": _strip.get_node("SettingsTab"),
}


## The contents learn the shell before their own _ready runs (children are ready first).
func _enter_tree() -> void:
	if shell != null:
		$Center/Page/Body/FieldGuide.shell = shell
		$Center/Page/Body/Settings.shell = shell


func _ready() -> void:
	for id in TABS:
		buttons[id].pressed.connect(_on_tab_pressed.bind(id))
	if shell == null:
		return
	visible = false
	# The live world shows through the scrim; the picture is the editor's stand-in.
	_world_picture.visible = false


## Show the page on tab `id`. Without a world the Field guide tab is off, and a request for
## it shows Settings.
func open(id: String, has_world: bool) -> void:
	buttons["guide"].disabled = not has_world
	visible = true
	_refresh()
	show_tab(id)


func close() -> void:
	visible = false
	guide.close()
	settings.visible = false


## Whether tab `id` can be shown now.
func enabled(id: String) -> bool:
	return buttons.has(id) and not buttons[id].disabled


## Show tab `id`'s content (and only it), its first control focused.
func show_tab(id: String) -> void:
	if not enabled(id):
		id = "settings" if id == "guide" else tab
		if not enabled(id):
			return
	tab = id
	# set_pressed_no_signal doesn't release the group's other buttons.
	for other in buttons:
		buttons[other].set_pressed_no_signal(other == id)
		buttons[other].queue_redraw()
	_hints.get_node("SelectHint").visible = id == "guide"
	_hints.get_node("ChangeHint").visible = id == "settings"
	if id == "guide":
		settings.visible = false
		guide.open()
	else:
		guide.close()
		var m: Dictionary = shell.current_motion() if shell != null else {"movement": settings.movement, "subpixel": settings.subpixel}
		settings.open(m["movement"], m["subpixel"])


## LB / RB: the next enabled tab to the left (-1) or right (+1); none past either end.
func step(dir: int) -> void:
	var i := TABS.find(tab) + dir
	while i >= 0 and i < TABS.size():
		if enabled(TABS[i]):
			show_tab(TABS[i])
			return
		i += dir


func _on_tab_pressed(id: String) -> void:
	if id != tab or not (guide.visible or settings.visible):
		show_tab(id)


func _process(delta: float) -> void:
	if not visible or shell == null:
		return
	# Paused follows the session every frame (it lags a pause or resume by a frame or so).
	_clock.get_node("Paused").visible = shell.session.status().get("paused", false)
	_refresh_left -= delta
	if _refresh_left <= 0.0:
		_refresh()


## The clock: day and time from `query("clock")` (hidden with no world), Paused from
## `session.status()`.
func _refresh() -> void:
	_refresh_left = REFRESH_S
	var s = shell.session if shell != null else null
	if s == null:
		return
	_clock.get_node("Paused").visible = s.status().get("paused", false)
	# No world (the Settings tab before the first one): no day or time.
	var clock = s.query("clock", null) if shell.has_world() else null
	_clock.get_node("DayLabel").visible = clock is Dictionary
	_clock.get_node("Day").visible = clock is Dictionary
	_clock.get_node("Time").visible = clock is Dictionary
	if clock is Dictionary:
		_clock.get_node("Day").text = str(int(clock.get("day", 0)))
		var hour: float = clock.get("hour", 0.0)
		_clock.get_node("Time").text = "%02d:%02d" % [int(hour) % 24, int(fmod(hour, 1.0) * 60.0)]
