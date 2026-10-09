@tool
class_name NeonPixels
extends Control
## A small pixel icon drawn as squares: round 3's search magnifier and group chevron. Each
## entry of `pixels` is one lit cell (column, row); cells are pixel_size px.

@export var pixels := PackedVector2Array():
	set(v):
		pixels = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 16, 1, "suffix:px") var pixel_size := 2:
	set(v):
		pixel_size = v
		update_minimum_size()
		queue_redraw()
@export var color := Color("#A595D8"):
	set(v):
		color = v
		queue_redraw()


func _cells() -> Vector2:
	var m := Vector2.ZERO
	for p in pixels:
		m = m.max(p + Vector2.ONE)
	return m


func _get_minimum_size() -> Vector2:
	return _cells() * pixel_size


func _draw() -> void:
	var oy := floorf((size.y - _cells().y * pixel_size) / 2.0)
	for p in pixels:
		draw_rect(Rect2(p.x * pixel_size, oy + p.y * pixel_size, pixel_size, pixel_size), color)
