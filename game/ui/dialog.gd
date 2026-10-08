extends Control
## A modal question over everything else: a confirmation, or a name to type. One at a
## time; Esc (or the gamepad's B) cancels it, Enter in the name field confirms.

var _message: Label
var _field: LineEdit
var _problem: Label
var _ok: Button
var _cancel: Button
var _on_ok: Callable


func _init() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	visible = false
	var dim := Panel.new()
	dim.theme_type_variation = &"Dim"
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)
	var centre := CenterContainer.new()
	centre.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(centre)
	var panel := PanelContainer.new()
	panel.custom_minimum_size = Vector2(520, 0)
	centre.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)
	_message = Label.new()
	_message.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(_message)
	_field = LineEdit.new()
	_field.text_submitted.connect(func(_t): _confirm())
	box.add_child(_field)
	_problem = Label.new()
	_problem.theme_type_variation = &"Warn"
	_problem.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(_problem)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_END
	box.add_child(row)
	_cancel = Button.new()
	_cancel.text = "Cancel"
	_cancel.pressed.connect(cancel)
	row.add_child(_cancel)
	_ok = Button.new()
	_ok.pressed.connect(_confirm)
	row.add_child(_ok)


func is_open() -> bool:
	return visible


## Ask. `on_ok` takes the typed text ("" without a field) and returns "" to close, or a
## problem to show and stay open.
func ask(message: String, ok_text: String, on_ok: Callable, field := false, text := "") -> void:
	_message.text = message
	_ok.text = ok_text
	_on_ok = on_ok
	_field.visible = field
	_field.text = text
	_problem.text = ""
	_problem.visible = false
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
	var problem: String = _on_ok.call(_field.text if _field.visible else "") if _on_ok.is_valid() else ""
	if problem != "":
		_problem.text = problem
		_problem.visible = true
		return
	visible = false
