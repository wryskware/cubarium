extends Control
## The pause menu: Resume, Save, Load, New world, Quit. The shell (main.gd) owns what each
## does and when the menu shows; Esc never quits.

signal chosen(item: String)

const ITEMS := [["resume", "Resume"], ["save", "Save"], ["load", "Load"], ["new", "New world"], ["quit", "Quit"]]

var buttons := {}
var _title: Label


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
	panel.custom_minimum_size = Vector2(320, 0)
	centre.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)
	_title = Label.new()
	_title.text = "Paused"
	_title.theme_type_variation = &"Title"
	_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(_title)
	for item in ITEMS:
		var b := Button.new()
		b.text = item[1]
		b.custom_minimum_size = Vector2(0, 44)
		b.pressed.connect(chosen.emit.bind(item[0]))
		box.add_child(b)
		buttons[item[0]] = b


## Show it. Without a running world, Resume and Save are off.
func open(has_world: bool) -> void:
	buttons["resume"].disabled = not has_world
	buttons["save"].disabled = not has_world
	_title.text = "Paused" if has_world else "Cubarium"
	visible = true
	focus_first()


func focus_first() -> void:
	for item in ITEMS:
		var b: Button = buttons[item[0]]
		if not b.disabled:
			b.grab_focus()
			return
