@tool
class_name NeonDotField
extends Control
## A field of unlit dots drawn as rects on whole device pixels, so it can't beat against
## the screen the way a font's own sub-pixel unlit dots do (chrome-dots trial, 2026-10-11).
## Dot centres sit at `first` + k × `pitch` inside the control's rect; each dot is a
## `dot_size` square snapped to the nearest whole pixel in canvas space, whatever the
## control's own (possibly fractional) position.

@export var color := Color(0.97, 0.94, 1.0, 0.5):
	set(v):
		color = v
		queue_redraw()
## Each dot's side in px.
@export_range(1, 16, 1, "suffix:px") var dot_size := 1:
	set(v):
		dot_size = v
		queue_redraw()
## Distance between dot centres in px (the font's dot pitch at its size).
@export_range(1, 64, 1, "suffix:px") var pitch := 2:
	set(v):
		pitch = v
		queue_redraw()
## The first dot's centre in the control's own px; may be fractional (a font's dot centre
## on a pixel corner is a whole number, on a pixel centre a half).
@export var first := Vector2(1, 1):
	set(v):
		first = v
		queue_redraw()


func _draw() -> void:
	if color.a <= 0.0 or pitch <= 0:
		return
	var o := get_global_transform_with_canvas().origin
	var half := dot_size / 2.0
	var y := first.y
	while y < size.y:
		var x := first.x
		while x < size.x:
			# snap the dot's top-left to a whole canvas pixel (centre on a corner: the
			# pixel right and below it)
			var g := (o + Vector2(x, y) - Vector2(half, half) + Vector2(0.501, 0.501)).floor()
			draw_rect(Rect2(g - o, Vector2(dot_size, dot_size)), color)
			x += pitch
		y += pitch
