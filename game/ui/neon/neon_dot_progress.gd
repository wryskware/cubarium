@tool
class_name NeonDotProgress
extends Control
## Round 4's progress (design/ui/design-language.md, "Settings controls (round 4)"): a row
## of the slider's 6 px dots on a 9 px pitch, done in cyan, the newest dot white, the rest
## unlit, with the percentage to the right in chrome (switch it off with show_percent). The
## small version, for terrain cards, uses 3 px dots on a 6 px pitch. The row fills the
## control's width, less the percentage's column. Every value below is an Inspector field.

## How far along, 0 to 1.
@export_range(0.0, 1.0, 0.001) var value := 0.62:
	set(v):
		value = clampf(v, 0.0, 1.0)
		queue_redraw()
@export var show_percent := true:
	set(v):
		show_percent = v
		update_minimum_size()
		queue_redraw()
## The card size: small_dot_size dots on small_pitch.
@export var small := false:
	set(v):
		small = v
		update_minimum_size()
		queue_redraw()
## No known end: a short run of lit dots sweeps along the row instead of a fill, and the
## percentage is hidden.
@export var indeterminate := false:
	set(v):
		indeterminate = v
		set_process(indeterminate)
		queue_redraw()

@export_group("Dots")
@export_range(1, 32, 1, "suffix:px") var dot_size := 6:
	set(v):
		dot_size = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 64, 1, "suffix:px") var pitch := 9:
	set(v):
		pitch = v
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var small_dot_size := 3:
	set(v):
		small_dot_size = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 64, 1, "suffix:px") var small_pitch := 6:
	set(v):
		small_pitch = v
		queue_redraw()
@export var done_color := Color("#42C5F8"):
	set(v):
		done_color = v
		queue_redraw()
## The newest lit dot.
@export var edge_color := Color.WHITE:
	set(v):
		edge_color = v
		queue_redraw()
@export var unlit_color := Color(165 / 255.0, 149 / 255.0, 216 / 255.0, 0.22):
	set(v):
		unlit_color = v
		queue_redraw()

@export_group("Percent")
@export var font: Font = preload("res://ui/neon/fonts/chrome.tres"):
	set(v):
		font = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 200, 1, "suffix:px") var font_size := 26:
	set(v):
		font_size = v
		update_minimum_size()
		queue_redraw()
@export var percent_color := Color("#DCD1F7"):
	set(v):
		percent_color = v
		queue_redraw()
## The percentage's column; the text sits at its right edge.
@export_range(0, 400, 1, "suffix:px") var percent_width := 78:
	set(v):
		percent_width = v
		update_minimum_size()
		queue_redraw()
## Between the row and the percentage's column.
@export_range(0, 200, 1, "suffix:px") var percent_gap := 18:
	set(v):
		percent_gap = v
		update_minimum_size()
		queue_redraw()

@export_group("Sweep")
## Lit dots in the indeterminate run.
@export_range(1, 64, 1) var sweep_dots := 8
## The run's speed in dots a second.
@export_range(1.0, 200.0, 0.5) var sweep_speed := 20.0

var _sweep := 0.0


func _ready() -> void:
	set_process(indeterminate)


func _process(delta: float) -> void:
	_sweep += delta * sweep_speed
	queue_redraw()


func _percent_shown() -> bool:
	return show_percent and not indeterminate


func _get_minimum_size() -> Vector2:
	var d := small_dot_size if small else dot_size
	var h := float(d)
	var w := 0.0
	if show_percent and font != null:
		h = maxf(h, font.get_height(font_size))
		w = percent_gap + percent_width
	return Vector2(w, h)


func _draw() -> void:
	var d := small_dot_size if small else dot_size
	var p := small_pitch if small else pitch
	var row_w := size.x - ((percent_gap + percent_width) if show_percent else 0)
	var n := maxi(1, int(floor((row_w + p - d) / float(p))))
	var y := floorf((size.y - d) / 2.0)
	if indeterminate:
		var span := n + sweep_dots
		var head := int(floor(_sweep)) % span
		for i in n:
			var lit := i <= head and i > head - sweep_dots
			draw_rect(Rect2(i * p, y, d, d), (edge_color if i == head else done_color) if lit else unlit_color)
	else:
		var k := int(round(value * n))
		for i in n:
			var c := unlit_color
			if i < k - 1:
				c = done_color
			elif i == k - 1:
				c = edge_color
			draw_rect(Rect2(i * p, y, d, d), c)
	if _percent_shown() and font != null:
		var text := "%d%%" % int(round(value * 100.0))
		var baseline := roundf(size.y / 2.0 + (font.get_ascent(font_size) - font.get_descent(font_size)) / 2.0)
		draw_string(font, Vector2(size.x - percent_width, baseline), text, HORIZONTAL_ALIGNMENT_RIGHT,
				percent_width, font_size, percent_color)
