extends Control
## The pause menu: Resume, Save, Load, New world, Field guide, Settings, Dev metrics, Quit. The shell (main.gd) owns what each
## does and when the menu shows; Esc never quits.
##
## The look is pause_menu.tscn, laid out in the editor from ui/neon/ (the field guide's
## frame, scrim and buttons): this script only wires it. Each item is the NeonButton under
## Items named after its id ("new" is New, "guide" is Guide, ...). Without a shell (opened
## in the editor, or by itself) the scene's sample stays as it is.

signal chosen(item: String)

const ITEMS := [["resume", "Resume"], ["save", "Save"], ["load", "Load"], ["new", "New world"], ["guide", "Field guide"], ["settings", "Settings"], ["metrics", "Dev metrics"], ["quit", "Quit"]]

## The shell (main.gd); null in the editor, which keeps the sample.
var shell: Node
var buttons := {}
var startup := false

@onready var _world: Control = $World
@onready var _title: NeonTitle = $Center/Page/Body/Margin/Stack/Title
@onready var _body: Control = $Center/Page/Body
@onready var _items: Control = $Center/Page/Body/Margin/Stack/Items


func _ready() -> void:
	for item in ITEMS:
		var b: BaseButton = _items.get_node(String(item[0]).capitalize())
		b.pressed.connect(chosen.emit.bind(item[0]))
		buttons[item[0]] = b
	if shell == null:
		return
	visible = false
	# The live world shows through the scrim; the picture is the editor's stand-in.
	_world.visible = false


## The title hub offers Continue, Load, New world, Settings and Quit; a running world
## keeps the ordinary pause hub and Resume.
func open(has_world: bool, continue_slot := "") -> void:
	startup = not has_world
	_body.offset_left = -430.0 if startup else -286.0
	_body.offset_right = 430.0 if startup else 286.0
	_body.offset_top = -340.0 if startup else -390.0
	_body.offset_bottom = 340.0 if startup else 390.0
	buttons["resume"].get_node("Content/Label").text = " Resume " if has_world else " Continue "
	buttons["resume"].disabled = not has_world and continue_slot == ""
	for item in ["save", "guide", "metrics"]:
		buttons[item].visible = has_world
	_world.visible = not has_world
	buttons["save"].disabled = not has_world
	buttons["metrics"].disabled = not has_world
	buttons["guide"].disabled = not has_world
	_title.title = "Paused" if has_world else "Cubarium"
	visible = true
	focus_first()


func focus_first() -> void:
	for item in ITEMS:
		var b: BaseButton = buttons[item[0]]
		if b.visible and not b.disabled:
			b.grab_focus()
			return
