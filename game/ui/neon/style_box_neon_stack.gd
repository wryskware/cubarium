@tool
class_name StyleBoxNeonStack
extends StyleBox
## Draws several styles on top of each other, first to last. The theme uses it where one
## state looks like two others together: hover is the normal style plus the focus glow
## (hover isn't designed yet; it matches focus for now). Its own content margins apply.

@export var layers: Array[StyleBox] = []:
	set(v):
		for s in layers:
			if s != null and s.changed.is_connected(emit_changed):
				s.changed.disconnect(emit_changed)
		layers = v
		for s in layers:
			if s != null and not s.changed.is_connected(emit_changed):
				s.changed.connect(emit_changed)
		emit_changed()


func _draw(to_canvas_item: RID, rect: Rect2) -> void:
	for s in layers:
		if s != null:
			s.draw(to_canvas_item, rect)


func _get_draw_rect(rect: Rect2) -> Rect2:
	var r := rect
	for s in layers:
		if s != null and s.has_method("_get_draw_rect"):
			r = r.merge(s._get_draw_rect(rect))
	return r
