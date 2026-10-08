extends Control
## The Settings page: how the animals move. **Movement**: Smooth (they glide between
## ticks) or Stepped (they hop a voxel at a time and turn in steps, as the old stamped
## animals did). **Subpixel** placement, for Smooth only: on, they glide between pixels;
## off, they move a whole pixel at a time. A change applies to the running world at once
## (`changed`); the shell keeps it for this viewer. Turn counts and blends stay in the
## presets and the config.

signal back
signal changed(movement: String, subpixel: bool)

var movement := "smooth"
var subpixel := true

var _smooth: Button
var _stepped: Button
var _movement_note: Label
var _subpixel: CheckBox
var _subpixel_note: Label
var _back: Button


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
	panel.custom_minimum_size = Vector2(440, 0)
	centre.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)
	var title := Label.new()
	title.text = "Settings"
	title.theme_type_variation = &"Title"
	box.add_child(title)

	var label := Label.new()
	label.text = "Animal movement"
	box.add_child(label)
	var row := HBoxContainer.new()
	box.add_child(row)
	var group := ButtonGroup.new()
	_smooth = _choice(row, group, "Smooth", "smooth")
	_stepped = _choice(row, group, "Stepped", "stepped")
	_movement_note = _note(box)

	_subpixel = CheckBox.new()
	_subpixel.text = "Subpixel placement"
	_subpixel.custom_minimum_size = Vector2(0, 40)
	_subpixel.toggled.connect(func(on: bool):
		subpixel = on
		_sync()
		changed.emit(movement, subpixel))
	box.add_child(_subpixel)
	_subpixel_note = _note(box)

	var bottom := HBoxContainer.new()
	box.add_child(bottom)
	_back = Button.new()
	_back.text = "Back"
	_back.custom_minimum_size = Vector2(120, 40)
	_back.pressed.connect(back.emit)
	bottom.add_child(_back)
	_sync()


func _choice(parent: Control, group: ButtonGroup, text: String, value: String) -> Button:
	var b := Button.new()
	b.text = text
	b.toggle_mode = true
	b.button_group = group
	b.custom_minimum_size = Vector2(140, 44)
	b.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	b.toggled.connect(func(on: bool):
		if on and movement != value:
			movement = value
			_sync()
			changed.emit(movement, subpixel))
	parent.add_child(b)
	return b


func _note(parent: Control) -> Label:
	var l := Label.new()
	l.theme_type_variation = &"Small"
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	parent.add_child(l)
	return l


## Show it with the settings in force.
func open(p_movement: String, p_subpixel: bool) -> void:
	set_values(p_movement, p_subpixel)
	visible = true
	focus_first()


## Show these settings without reporting a change.
func set_values(p_movement: String, p_subpixel: bool) -> void:
	movement = "stepped" if p_movement == "stepped" else "smooth"
	subpixel = p_subpixel
	_sync()


func focus_first() -> void:
	(_stepped if movement == "stepped" else _smooth).grab_focus()


## Movement: Smooth or Stepped, as a viewer would press it.
func choose_movement(value: String) -> void:
	(_stepped if value == "stepped" else _smooth).button_pressed = true


## Subpixel on or off, as a viewer would tick it (nothing under Stepped, where it is off).
func choose_subpixel(on: bool) -> void:
	if not _subpixel.disabled:
		_subpixel.button_pressed = on


func subpixel_enabled() -> bool:
	return not _subpixel.disabled


func _sync() -> void:
	_smooth.set_pressed_no_signal(movement == "smooth")
	_stepped.set_pressed_no_signal(movement == "stepped")
	_subpixel.set_pressed_no_signal(subpixel)
	_subpixel.disabled = movement == "stepped"
	_movement_note.text = "They hop a voxel at a time and turn in steps." if movement == "stepped" \
		else "They glide between ticks."
	_subpixel_note.text = "Stepped animals sit on whole pixels." if movement == "stepped" \
		else ("They glide between pixels." if subpixel else "They move a whole pixel at a time.")
