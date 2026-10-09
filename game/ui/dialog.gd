extends Control
## A modal question over everything else: a confirmation, or a name to type. One at a
## time; Esc (or the gamepad's B) cancels it, Enter in the name field confirms.
##
## The look is dialog.tscn, laid out in the editor from ui/neon/ (the name field is the
## field guide's search box): this script only fills it. Without a shell (opened in the
## editor, or by itself) the scene's sample, the name prompt with a problem, stays.

const GLOW := preload("res://ui/neon/styles/glow_focus.tres")

## The shell (main.gd); null in the editor, which keeps the sample.
var shell: Node

var _on_ok: Callable
var _has_field := false

@onready var _world: Control = $World
@onready var _stack: Control = $Center/Page/Body/Margin/Stack
@onready var _message: Label = _stack.get_node("Message")
@onready var _field_box: Control = _stack.get_node("FieldMargin")
@onready var _name_box: Control = _stack.get_node("FieldMargin/NameBox")
@onready var _field: LineEdit = _stack.get_node("FieldMargin/NameBox/Field")
@onready var _problem_box: Control = _stack.get_node("ProblemMargin")
@onready var _problem: Label = _stack.get_node("ProblemMargin/Problem")
@onready var _cancel: BaseButton = _stack.get_node("ActionsMargin/Actions/Cancel")
@onready var _ok: BaseButton = _stack.get_node("ActionsMargin/Actions/Ok")
@onready var _ok_label: Label = _ok.get_node("Content/Label")


func _ready() -> void:
	_field.text_submitted.connect(func(_t): _confirm())
	# The field draws no focus of its own: its box glows, as the guide's search box does.
	_field.focus_entered.connect(_name_box.queue_redraw)
	_field.focus_exited.connect(_name_box.queue_redraw)
	_name_box.draw.connect(_draw_field_focus)
	_cancel.pressed.connect(cancel)
	_ok.pressed.connect(_confirm)
	_has_field = _field_box.visible
	if shell == null:
		return
	visible = false
	# The live world (or the screen under the question) shows through the scrim.
	_world.visible = false


func _draw_field_focus() -> void:
	if _field.has_focus():
		_name_box.draw_style_box(GLOW, Rect2(Vector2.ZERO, _name_box.size))


func is_open() -> bool:
	return visible


## Ask. `on_ok` takes the typed text ("" without a field) and returns "" to close, or a
## problem to show and stay open.
func ask(message: String, ok_text: String, on_ok: Callable, field := false, text := "") -> void:
	_message.text = message
	_ok_label.text = " %s " % ok_text
	_on_ok = on_ok
	_has_field = field
	_field_box.visible = field
	_field.text = text
	_problem.text = ""
	_problem_box.visible = false
	visible = true
	if field:
		_field.grab_focus()
		_field.select_all()
	else:
		_cancel.grab_focus()


func cancel() -> void:
	visible = false
	_on_ok = Callable()


func _confirm() -> void:
	if not visible:
		return
	var problem: String = _on_ok.call(_field.text if _has_field else "") if _on_ok.is_valid() else ""
	if problem != "":
		_problem.text = problem
		_problem_box.visible = true
		return
	visible = false
