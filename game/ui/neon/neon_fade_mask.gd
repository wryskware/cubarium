@tool
class_name NeonFadeMask
extends Control
## Fades its children out over its bottom `fade` px, as round 3's scrolling entry does
## above the fixed buttons. It draws an opaque mask that ends in a gradient, and
## clip_children (Clip Only) turns that into its children's alpha. Give it one child that
## fills it, such as a ScrollContainer.
##
## `fade_top` fades the top edge the same way. With `follow_scroll` on, each edge fades only
## while its ScrollContainer child can scroll further that way (the field guide's species
## list: no fade over a list that fits, a top fade once it is scrolled down); the fade
## grows with the distance left to scroll, up to its full size, so it never pops.

@export_range(0, 400, 1, "suffix:px") var fade := 36:
	set(v):
		fade = v
		queue_redraw()
@export_range(0, 400, 1, "suffix:px") var fade_top := 0:
	set(v):
		fade_top = v
		queue_redraw()
@export var follow_scroll := false:
	set(v):
		follow_scroll = v
		_watch()
		queue_redraw()

var _bar: ScrollBar


func _init() -> void:
	clip_children = CanvasItem.CLIP_CHILDREN_ONLY


func _ready() -> void:
	_watch()


func _watch() -> void:
	if not is_node_ready():
		return
	var scroll: ScrollContainer = null
	for c in get_children():
		if c is ScrollContainer:
			scroll = c
			break
	var bar := scroll.get_v_scroll_bar() if scroll != null and follow_scroll else null
	if bar == _bar:
		return
	if _bar != null:
		_bar.value_changed.disconnect(_on_scrolled)
		_bar.changed.disconnect(queue_redraw)
	_bar = bar
	if _bar != null:
		_bar.value_changed.connect(_on_scrolled)
		_bar.changed.connect(queue_redraw)


func _on_scrolled(_v: float) -> void:
	queue_redraw()


## The two fades in force, px: (top, bottom).
func fades() -> Vector2:
	var top := float(fade_top)
	var bottom := float(fade)
	if follow_scroll and _bar != null:
		var above := _bar.value - _bar.min_value
		var below := _bar.max_value - _bar.page - _bar.value
		top = minf(top, maxf(above, 0.0))
		bottom = minf(bottom, maxf(below, 0.0))
	elif follow_scroll:
		top = 0.0
		bottom = 0.0
	return Vector2(minf(top, size.y), minf(bottom, size.y))


func _draw() -> void:
	var f := fades()
	var t := f.x
	var b := f.y
	var clear := Color(1, 1, 1, 0)
	if t > 0.0:
		draw_polygon(PackedVector2Array([Vector2(0, 0), Vector2(size.x, 0), Vector2(size.x, t), Vector2(0, t)]),
				PackedColorArray([clear, clear, Color.WHITE, Color.WHITE]))
	var y := size.y - b
	if y > t:
		draw_rect(Rect2(0, t, size.x, y - t), Color.WHITE)
	if b > 0.0:
		draw_polygon(PackedVector2Array([Vector2(0, y), Vector2(size.x, y), Vector2(size.x, size.y), Vector2(0, size.y)]),
				PackedColorArray([Color.WHITE, Color.WHITE, clear, clear]))
