@tool
class_name NeonCheckbox
extends BaseButton
## Round 4's on/off (design/ui/design-language.md, "Settings controls (round 4)"): a 33 px
## notched box with its text beside it in body text. Off is the line colour, empty; on is a
## hot ring filled with the selected-chip dot plate, with a white pixel tick. A or Enter, or
## a click, toggles it (`toggled`, as any BaseButton in toggle mode). Focus glows around the
## box and the text. Every value below is an Inspector field.

## Beside the box.
@export var text := "Glide between pixels":
	set(v):
		text = v
		update_minimum_size()
		queue_redraw()
## Draws the focus glow without focus, for galleries that show every state at once.
@export var preview_focus := false:
	set(v):
		preview_focus = v
		queue_redraw()
## Drawn around the box and the text while focused.
@export var focus_style: StyleBox = preload("res://ui/neon/styles/glow_focus.tres"):
	set(v):
		focus_style = v
		queue_redraw()
## Percent of its opacity the control keeps while disabled.
@export_range(0, 100, 1, "suffix:%") var disabled_opacity := 40:
	set(v):
		disabled_opacity = v
		queue_redraw()

@export_group("Layout")
@export_range(1, 200, 1, "suffix:px") var height := 52:
	set(v):
		height = v
		update_minimum_size()
		queue_redraw()
@export_range(0, 100, 1, "suffix:px") var pad_left := 9:
	set(v):
		pad_left = v
		update_minimum_size()
		queue_redraw()
@export_range(0, 100, 1, "suffix:px") var pad_right := 12:
	set(v):
		pad_right = v
		update_minimum_size()
		queue_redraw()
## Between the box and the text.
@export_range(0, 100, 1, "suffix:px") var gap := 16:
	set(v):
		gap = v
		update_minimum_size()
		queue_redraw()

@export_group("Box")
@export_range(1, 200, 1, "suffix:px") var box_size := 33:
	set(v):
		box_size = v
		update_minimum_size()
		queue_redraw()
## The notched ring, drawn outside the box.
@export_range(0, 16, 1, "suffix:px") var ring_width := 3:
	set(v):
		ring_width = v
		queue_redraw()
@export var off_ring_color := Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.32):
	set(v):
		off_ring_color = v
		queue_redraw()
@export var off_fill_color := Color(11 / 255.0, 5 / 255.0, 37 / 255.0, 0.5):
	set(v):
		off_fill_color = v
		queue_redraw()
@export var on_ring_color := Color("#F25BDB"):
	set(v):
		on_ring_color = v
		queue_redraw()
## The selected chip's plate: dots of dot_size on a dot_pitch grid.
@export var on_dot_color := Color(242 / 255.0, 91 / 255.0, 219 / 255.0, 0.65):
	set(v):
		on_dot_color = v
		queue_redraw()
@export_range(1, 16, 1, "suffix:px") var dot_size := 2:
	set(v):
		dot_size = v
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var dot_pitch := 4:
	set(v):
		dot_pitch = v
		queue_redraw()
@export var tick_color := Color.WHITE:
	set(v):
		tick_color = v
		queue_redraw()
## Screen px per tick pixel.
@export_range(1, 16, 1, "suffix:px") var tick_pixel := 3:
	set(v):
		tick_pixel = v
		queue_redraw()

@export_group("Text")
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
@export var text_color := Color("#DCD1F7"):
	set(v):
		text_color = v
		queue_redraw()

## Round 4's tick, 7×6 cells (column, row).
const TICK := [Vector2(6, 0), Vector2(5, 1), Vector2(4, 2), Vector2(0, 2), Vector2(1, 3), Vector2(3, 3), Vector2(2, 4)]
const TICK_CELLS := Vector2(7, 6)


func _init() -> void:
	toggle_mode = true
	focus_mode = Control.FOCUS_ALL


func _ready() -> void:
	focus_entered.connect(queue_redraw)
	focus_exited.connect(queue_redraw)


## Disabled: greyed and out of the focus order (BaseButton has no hook for `disabled`, but
## setting it redraws).
func _sync_disabled() -> void:
	modulate.a = disabled_opacity / 100.0 if disabled else 1.0
	focus_mode = Control.FOCUS_NONE if disabled else Control.FOCUS_ALL
	if disabled and is_inside_tree() and has_focus():
		release_focus()


func _get_minimum_size() -> Vector2:
	var w := pad_left + box_size + pad_right
	if font != null and text != "":
		w += gap + font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x
	return Vector2(ceilf(w), height)


func _draw() -> void:
	var keep := disabled_opacity / 100.0 if disabled else 1.0
	if not is_equal_approx(modulate.a, keep) or (focus_mode == Control.FOCUS_NONE) != disabled:
		_sync_disabled.call_deferred()
	var on := button_pressed
	var b := Rect2(pad_left, floorf((size.y - box_size) / 2.0), box_size, box_size)
	draw_rect(b, off_fill_color)
	if on:
		var y := 0
		while y < box_size:
			var x := 0
			while x < box_size:
				draw_rect(Rect2(b.position + Vector2(x, y), Vector2(mini(dot_size, box_size - x), mini(dot_size, box_size - y))), on_dot_color)
				x += dot_pitch
			y += dot_pitch
		var t := (b.size - TICK_CELLS * tick_pixel) / 2.0
		var origin := (b.position + t).floor()
		for c in TICK:
			draw_rect(Rect2(origin + c * tick_pixel, Vector2(tick_pixel, tick_pixel)), tick_color)
	var ring := on_ring_color if on else off_ring_color
	if ring_width > 0:
		var w := float(ring_width)
		draw_rect(Rect2(b.position.x, b.position.y - w, b.size.x, w), ring)
		draw_rect(Rect2(b.position.x, b.end.y, b.size.x, w), ring)
		draw_rect(Rect2(b.position.x - w, b.position.y, w, b.size.y), ring)
		draw_rect(Rect2(b.end.x, b.position.y, w, b.size.y), ring)
	if font != null and text != "":
		var baseline := roundf(size.y / 2.0 + (font.get_ascent(font_size) - font.get_descent(font_size)) / 2.0)
		draw_string(font, Vector2(b.end.x + gap, baseline), text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size, text_color)
	if (has_focus() or preview_focus) and not disabled and focus_style != null:
		draw_style_box(focus_style, Rect2(Vector2.ZERO, size))
