@tool
class_name NeonArrows
extends Control
## Round 4's choice from a list (design/ui/design-language.md, "Settings controls (round
## 4)"): the value centred in an inner box with a 3×5 pixel chevron at 4× on each side, and
## a row of pips under it, one per option, the current one in hot. Left and right (keys,
## d-pad, stick) step through the options; a click on either arrow steps too. Focus is the
## glow around the box. Every value below is an Inspector field.
##
## `index` set from code moves quietly; `step` and `choose` act as a viewer would and emit
## `changed`.

signal changed(index: int)

## The options, in order.
@export var options := PackedStringArray(["Terrarium", "Deep 1.5×", "Deep 2×", "Wide"]):
	set(v):
		options = v
		index = index
		update_minimum_size()
		queue_redraw()
## The current option.
@export var index := 0:
	set(v):
		index = clampi(v, 0, maxi(options.size() - 1, 0))
		queue_redraw()
## Right at the last option goes to the first, and left at the first to the last.
@export var wraps := true
## Greyed to disabled_opacity, not focusable, deaf to input.
@export var disabled := false:
	set(v):
		disabled = v
		_apply_disabled()
## Draws the focus glow without focus, for galleries that show every state at once.
@export var preview_focus := false:
	set(v):
		preview_focus = v
		queue_redraw()

@export_group("Box")
## The inner box: a notched `line` ring over rgba(11,5,37,.5).
@export var box_style: StyleBox = preload("res://ui/neon/styles/search_box.tres"):
	set(v):
		box_style = v
		queue_redraw()
## Drawn around the box while focused.
@export var focus_style: StyleBox = preload("res://ui/neon/styles/glow_focus.tres"):
	set(v):
		focus_style = v
		queue_redraw()
@export_range(0, 200, 1, "suffix:px") var box_height := 60:
	set(v):
		box_height = v
		update_minimum_size()
		queue_redraw()
## Percent of its opacity the control keeps while disabled.
@export_range(0, 100, 1, "suffix:%") var disabled_opacity := 40:
	set(v):
		disabled_opacity = v
		_apply_disabled()

@export_group("Value")
@export var font: Font = preload("res://ui/neon/fonts/text.tres"):
	set(v):
		font = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 200, 1, "suffix:px") var font_size := 21:
	set(v):
		font_size = v
		update_minimum_size()
		queue_redraw()
@export var value_color := Color("#F7F0FF"):
	set(v):
		value_color = v
		queue_redraw()
## The space kept under the value for the pips: the value centres in the box above it.
@export_range(0, 100, 1, "suffix:px") var value_bottom_pad := 12:
	set(v):
		value_bottom_pad = v
		queue_redraw()

@export_group("Arrows")
@export var arrow_color := Color("#A595D8"):
	set(v):
		arrow_color = v
		queue_redraw()
@export var arrow_focus_color := Color("#42C5F8"):
	set(v):
		arrow_focus_color = v
		queue_redraw()
## Screen px per chevron pixel.
@export_range(1, 16, 1, "suffix:px") var arrow_pixel := 4:
	set(v):
		arrow_pixel = v
		update_minimum_size()
		queue_redraw()
## From the box's edge to the chevron.
@export_range(0, 100, 1, "suffix:px") var arrow_inset := 16:
	set(v):
		arrow_inset = v
		update_minimum_size()
		queue_redraw()
## How far in from each edge a click counts as a click on that arrow.
@export_range(0, 400, 1, "suffix:px") var arrow_hit_width := 64

@export_group("Pips")
@export_range(1, 32, 1, "suffix:px") var pip_size := 6:
	set(v):
		pip_size = v
		queue_redraw()
@export_range(0, 32, 1, "suffix:px") var pip_gap := 6:
	set(v):
		pip_gap = v
		queue_redraw()
@export var pip_color := Color(165 / 255.0, 149 / 255.0, 216 / 255.0, 0.3):
	set(v):
		pip_color = v
		queue_redraw()
@export var pip_on_color := Color("#F25BDB"):
	set(v):
		pip_on_color = v
		queue_redraw()
## From the box's bottom edge to the pips' bottom.
@export_range(0, 100, 1, "suffix:px") var pip_bottom := 10:
	set(v):
		pip_bottom = v
		queue_redraw()

## Round 4's chevrons, 3×5 cells (column, row).
const LEFT := [Vector2(2, 0), Vector2(1, 1), Vector2(0, 2), Vector2(1, 3), Vector2(2, 4)]
const RIGHT := [Vector2(0, 0), Vector2(1, 1), Vector2(2, 2), Vector2(1, 3), Vector2(0, 4)]


func _init() -> void:
	focus_mode = Control.FOCUS_ALL
	mouse_filter = Control.MOUSE_FILTER_STOP


func _ready() -> void:
	_apply_disabled()
	focus_entered.connect(queue_redraw)
	focus_exited.connect(queue_redraw)


## As a viewer would: one option left (-1) or right (+1).
func step(delta: int) -> void:
	if disabled or options.is_empty():
		return
	var n := options.size()
	var i := index + delta
	i = posmod(i, n) if wraps else clampi(i, 0, n - 1)
	choose(i)


## As a viewer would: option `i`, reporting `changed` when it moves.
func choose(i: int) -> void:
	if disabled or options.is_empty():
		return
	i = clampi(i, 0, options.size() - 1)
	if i == index:
		return
	index = i
	changed.emit(index)


func value() -> String:
	return options[index] if index >= 0 and index < options.size() else ""


func _gui_input(event: InputEvent) -> void:
	if disabled:
		return
	if event.is_action_pressed("ui_left", true):
		step(-1)
		accept_event()
	elif event.is_action_pressed("ui_right", true):
		step(1)
		accept_event()
	elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var x: float = event.position.x
		if x < arrow_hit_width:
			step(-1)
			accept_event()
		elif x > size.x - arrow_hit_width:
			step(1)
			accept_event()


func _apply_disabled() -> void:
	modulate.a = disabled_opacity / 100.0 if disabled else 1.0
	focus_mode = Control.FOCUS_NONE if disabled else Control.FOCUS_ALL
	if disabled and is_inside_tree() and has_focus():
		release_focus()
	queue_redraw()


func _get_minimum_size() -> Vector2:
	var w := 2.0 * (arrow_inset + 3 * arrow_pixel)
	if font != null:
		var widest := 0.0
		for o in options:
			widest = maxf(widest, font.get_string_size(o, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x)
		w += widest + 24.0
	return Vector2(w, box_height)


func _draw() -> void:
	var r := Rect2(Vector2.ZERO, Vector2(size.x, box_height))
	if box_style != null:
		draw_style_box(box_style, r)
	var focused := (has_focus() or preview_focus) and not disabled
	var arrow := arrow_focus_color if focused else arrow_color
	var ay := floorf((box_height - 5 * arrow_pixel) / 2.0)
	_chevron(LEFT, Vector2(arrow_inset, ay), arrow)
	_chevron(RIGHT, Vector2(size.x - arrow_inset - 3 * arrow_pixel, ay), arrow)
	if font != null:
		var text := value()
		var middle := (box_height - value_bottom_pad) / 2.0
		var baseline := roundf(middle + (font.get_ascent(font_size) - font.get_descent(font_size)) / 2.0)
		var left := arrow_inset + 3.0 * arrow_pixel
		draw_string(font, Vector2(left, baseline), text, HORIZONTAL_ALIGNMENT_CENTER,
				size.x - 2.0 * left, font_size, value_color)
	var n := options.size()
	if n > 0:
		var row := n * pip_size + (n - 1) * pip_gap
		var px := roundf((size.x - row) / 2.0)
		var py := box_height - pip_bottom - pip_size
		for i in n:
			draw_rect(Rect2(px + i * (pip_size + pip_gap), py, pip_size, pip_size),
					pip_on_color if i == index else pip_color)
	if focused and focus_style != null:
		draw_style_box(focus_style, r)


func _chevron(cells: Array, at: Vector2, colour: Color) -> void:
	for c in cells:
		draw_rect(Rect2(at + c * arrow_pixel, Vector2(arrow_pixel, arrow_pixel)), colour)
