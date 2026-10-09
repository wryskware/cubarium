@tool
class_name NeonFadeMask
extends Control
## Fades its children out over its bottom `fade` px, as round 3's scrolling entry does
## above the fixed buttons. It draws an opaque mask that ends in a gradient, and
## clip_children (Clip Only) turns that into its children's alpha. Give it one child that
## fills it, such as a ScrollContainer.

@export_range(0, 400, 1, "suffix:px") var fade := 36:
	set(v):
		fade = v
		queue_redraw()


func _init() -> void:
	clip_children = CanvasItem.CLIP_CHILDREN_ONLY


func _draw() -> void:
	var f := minf(fade, size.y)
	var y := size.y - f
	if y > 0.0:
		draw_rect(Rect2(0, 0, size.x, y), Color.WHITE)
	if f > 0.0:
		draw_polygon(PackedVector2Array([Vector2(0, y), Vector2(size.x, y), Vector2(size.x, size.y), Vector2(0, size.y)]),
				PackedColorArray([Color.WHITE, Color.WHITE, Color(1, 1, 1, 0), Color(1, 1, 1, 0)]))
