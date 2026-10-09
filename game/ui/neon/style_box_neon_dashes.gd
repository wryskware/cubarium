@tool
class_name StyleBoxNeonDashes
extends StyleBox
## A dashed rule of square dashes along chosen edges, inside the rect: the facts' dividers and
## the closed group's top line (3 px dashes of `line`), and the hot rule under the binomial.
## Dashes start at the rect's left (or top) edge, as round 3's repeating gradients do.

@export var color := Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.32):
	set(v):
		color = v
		emit_changed()
@export_range(1, 32, 1, "suffix:px") var dash := 3:
	set(v):
		dash = v
		emit_changed()
@export_range(0, 32, 1, "suffix:px") var gap := 3:
	set(v):
		gap = v
		emit_changed()
@export_range(1, 32, 1, "suffix:px") var thickness := 3:
	set(v):
		thickness = v
		emit_changed()
@export var top := true:
	set(v):
		top = v
		emit_changed()
@export var bottom := false:
	set(v):
		bottom = v
		emit_changed()
@export var left := false:
	set(v):
		left = v
		emit_changed()
@export var right := false:
	set(v):
		right = v
		emit_changed()


func _draw(to_canvas_item: RID, rect: Rect2) -> void:
	if color.a <= 0.0:
		return
	if top:
		_row(to_canvas_item, rect.position, rect.size.x)
	if bottom:
		_row(to_canvas_item, Vector2(rect.position.x, rect.end.y - thickness), rect.size.x)
	if left:
		_column(to_canvas_item, rect.position, rect.size.y)
	if right:
		_column(to_canvas_item, Vector2(rect.end.x - thickness, rect.position.y), rect.size.y)


func _row(ci: RID, at: Vector2, length: float) -> void:
	var x := 0.0
	while x < length:
		RenderingServer.canvas_item_add_rect(ci, Rect2(at + Vector2(x, 0), Vector2(minf(dash, length - x), thickness)), color)
		x += dash + gap


func _column(ci: RID, at: Vector2, length: float) -> void:
	var y := 0.0
	while y < length:
		RenderingServer.canvas_item_add_rect(ci, Rect2(at + Vector2(0, y), Vector2(thickness, minf(dash, length - y))), color)
		y += dash + gap
