@tool
class_name StyleBoxNeonDots
extends StyleBox
## The neon UI's dot plate (design/ui/design-language.md, "Controls: lit dots"): square dots
## on a grid, starting at the rect's top-left corner, with an optional solid bar down the
## left edge. The selected list row is 2 px pink dots on a 4 px grid, rgba(242,91,219,.32),
## with a 6 px hot bar; chips are a faint plate, selected chips a brighter one.

@export var dot_color := Color(242 / 255.0, 91 / 255.0, 219 / 255.0, 0.32):
	set(v):
		dot_color = v
		emit_changed()
## Each dot's side in px.
@export_range(1, 16, 1, "suffix:px") var dot_size := 2:
	set(v):
		dot_size = v
		emit_changed()
## Distance between dot origins in px.
@export_range(1, 32, 1, "suffix:px") var pitch := 4:
	set(v):
		pitch = v
		emit_changed()

@export_group("Left bar")
@export var bar_color := Color("#F25BDB"):
	set(v):
		bar_color = v
		emit_changed()
## The bar's width in px; 0 draws no bar.
@export_range(0, 32, 1, "suffix:px") var bar_width := 0:
	set(v):
		bar_width = v
		emit_changed()


func _draw(to_canvas_item: RID, rect: Rect2) -> void:
	var rs := RenderingServer
	if dot_color.a > 0.0 and dot_size > 0 and pitch > 0:
		var y := 0
		while y < rect.size.y:
			var h := minf(dot_size, rect.size.y - y)
			var x := 0
			while x < rect.size.x:
				var w := minf(dot_size, rect.size.x - x)
				rs.canvas_item_add_rect(to_canvas_item, Rect2(rect.position + Vector2(x, y), Vector2(w, h)), dot_color)
				x += pitch
			y += pitch
	if bar_width > 0 and bar_color.a > 0.0:
		rs.canvas_item_add_rect(to_canvas_item, Rect2(rect.position, Vector2(minf(bar_width, rect.size.x), rect.size.y)), bar_color)
