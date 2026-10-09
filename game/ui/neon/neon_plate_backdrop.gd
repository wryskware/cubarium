@tool
class_name NeonPlateBackdrop
extends Control
## The field-guide plate's backdrop behind the specimen art (round 1 to 3's plate): a
## banded sun cut by horizontal gaps, and horizon lines that fade in towards the bottom.

@export_group("Sun")
## Band colours, top to bottom.
@export var sun_colors := PackedColorArray([Color("#FDA243"), Color("#F98C78"), Color("#F25BDB"),
		Color("#B037C4"), Color("#5A1D9D")]):
	set(v):
		sun_colors = v
		queue_redraw()
## Where each band ends, as a fraction of the sun's height (one fewer than the colours).
@export var sun_edges := PackedFloat32Array([0.22, 0.38, 0.58, 0.78]):
	set(v):
		sun_edges = v
		queue_redraw()
## Cut-out stripes, each from x to y as fractions of the sun's height.
@export var sun_gaps := PackedVector2Array([Vector2(0.52, 0.56), Vector2(0.64, 0.69),
		Vector2(0.76, 0.82), Vector2(0.88, 1.0)]):
	set(v):
		sun_gaps = v
		queue_redraw()
@export_range(0, 1000, 1, "suffix:px") var sun_diameter := 300:
	set(v):
		sun_diameter = v
		queue_redraw()
## The sun's top, in px from the backdrop's top; it is centred across.
@export_range(-500, 1000, 1, "suffix:px") var sun_top := 70:
	set(v):
		sun_top = v
		queue_redraw()
@export_range(0.0, 1.0) var sun_alpha := 0.55:
	set(v):
		sun_alpha = v
		queue_redraw()

@export_group("Horizon")
@export var horizon_color := Color(242 / 255.0, 91 / 255.0, 219 / 255.0, 0.45):
	set(v):
		horizon_color = v
		queue_redraw()
## Height of the lined band at the bottom, which fades in from transparent at its top.
@export_range(0, 1000, 1, "suffix:px") var horizon_height := 130:
	set(v):
		horizon_height = v
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var line_width := 3:
	set(v):
		line_width = v
		queue_redraw()
@export_range(1, 200, 1, "suffix:px") var line_pitch := 24:
	set(v):
		line_pitch = v
		queue_redraw()


func _draw() -> void:
	_draw_sun()
	_draw_horizon()


func _draw_sun() -> void:
	var d := float(sun_diameter)
	if d <= 0.0 or sun_colors.is_empty():
		return
	var r := d / 2.0
	var cx := size.x / 2.0
	for y in sun_diameter:
		var f := (y + 0.5) / d
		var cut := false
		for g in sun_gaps:
			if f >= g.x and f < g.y:
				cut = true
		if cut:
			continue
		var c := sun_colors[sun_colors.size() - 1]
		for i in mini(sun_edges.size(), sun_colors.size() - 1):
			if f < sun_edges[i]:
				c = sun_colors[i]
				break
		c.a *= sun_alpha
		var dy := y + 0.5 - r
		var half := sqrt(maxf(0.0, r * r - dy * dy))
		var x0 := cx - half
		var x1 := cx + half
		var a := ceilf(x0)
		var b := floorf(x1)
		var top := sun_top + y
		if b > a:
			draw_rect(Rect2(a, top, b - a, 1), c)
		# the edge pixels by coverage, as the browser's smooth circle
		draw_rect(Rect2(a - 1, top, 1, 1), Color(c, c.a * (a - x0)))
		draw_rect(Rect2(b, top, 1, 1), Color(c, c.a * (x1 - b)))


func _draw_horizon() -> void:
	var hh := float(horizon_height)
	if hh <= 0.0:
		return
	var top := size.y - hh
	var y := 0
	while y < horizon_height:
		for k in line_width:
			if y + k < horizon_height:
				var fade := (y + k + 0.5) / hh
				draw_rect(Rect2(0, top + y + k, size.x, 1), Color(horizon_color, horizon_color.a * fade))
		y += line_pitch
