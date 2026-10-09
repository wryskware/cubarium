@tool
class_name NeonLabel
extends Label
## A Label whose look follows the NeonButton it sits in. While that button is pressed (held
## down, or toggled on: the selected tab, list row or chip) it uses the theme type
## variation pressed_look and adds pressed_pad to both ends of its text; otherwise it uses
## look. Set look and pressed_look here: theme_type_variation follows them.
##
## The selected tab is the case that needs both: Gridlite's negative setting with one
## space of lit plate on each side (design/ui/design-language.md, "Controls: lit dots").

@export var look: StringName:
	set(v):
		look = v
		_apply()
@export var pressed_look: StringName:
	set(v):
		pressed_look = v
		_apply()
## Text added to both ends while pressed, e.g. one space: Gridlite's lit plate shows there.
@export var pressed_pad := "":
	set(v):
		_unpad()
		pressed_pad = v
		_apply()

var _pressed := false


func set_pressed_look(on: bool) -> void:
	_pressed = on
	_apply()


func _apply() -> void:
	var want := pressed_look if _pressed and pressed_look != &"" else look
	if want != &"" and theme_type_variation != want:
		theme_type_variation = want
	if _pressed and pressed_pad != "":
		if not (text.begins_with(pressed_pad) and text.ends_with(pressed_pad) and text.length() >= 2 * pressed_pad.length()):
			text = pressed_pad + text + pressed_pad
	else:
		_unpad()


func _unpad() -> void:
	var p := pressed_pad
	if p != "" and text.length() >= 2 * p.length() and text.begins_with(p) and text.ends_with(p):
		text = text.substr(p.length(), text.length() - 2 * p.length())
