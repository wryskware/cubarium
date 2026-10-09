@tool
class_name StyleBoxNeonFocus
extends StyleBoxNeonFrame
## The neon UI's focus (design/ui/design-language.md, "Focus: glow"): a 3 px notched cyan
## line 6 px outside the focused thing, with an 18 px cyan glow, rgba(66,197,248,.55). It is
## the notched frame with no fill, moved out by `outset`; these are its starting values and
## every one stays an Inspector field. The theme uses one shared copy, styles/glow_focus.tres.


func _init() -> void:
	ring_color = Color("#42C5F8")
	ring_width = 3
	outset = 6
	glow_color = Color(66 / 255.0, 197 / 255.0, 248 / 255.0, 0.55)
	glow_blur = 18
	glow_spread = 2
