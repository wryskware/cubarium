extends Control
## **A line chart for the dev metrics panel**: series over simulated minutes, drawn in
## `_draw()` with the theme's own label font and colour. A `null` value is a gap (NA), never
## a zero. Each series is `{"name": String, "colour": Color, "points": Array}` with points
## `[minute, value_or_null]` in minute order. The legend (name and newest value, in the
## series' colour) sits above the plot.

const PAD := Vector2(64, 12)
const LEGEND_LINE := 20.0
const TICKS := 4

var series: Array = []
var title := ""


func _init() -> void:
	custom_minimum_size = Vector2(0, 240)
	size_flags_horizontal = Control.SIZE_EXPAND_FILL
	size_flags_vertical = Control.SIZE_EXPAND_FILL
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func set_series(s: Array, heading := "") -> void:
	series = s
	title = heading
	queue_redraw()


func _font() -> Font:
	return get_theme_font("font", &"Label")


func _font_size() -> int:
	return get_theme_font_size("font_size", &"Small")


func _ink() -> Color:
	return get_theme_color("font_color", &"Small")


## Minute and value bounds over every point: [x0, x1, y0, y1], or [] with none.
func bounds() -> Array:
	var b := []
	for s in series:
		for p in s["points"]:
			if p[1] == null:
				continue
			var x := float(p[0])
			var y := float(p[1])
			if b.is_empty():
				b = [x, x, y, y]
			else:
				b = [minf(b[0], x), maxf(b[1], x), minf(b[2], y), maxf(b[3], y)]
	if b.is_empty():
		return b
	if b[1] - b[0] < 1e-9:
		b[1] = b[0] + 1.0
	if b[3] - b[2] < 1e-12:
		var pad := maxf(absf(b[3]) * 0.1, 1.0)
		b[2] -= pad
		b[3] += pad
	return b


static func label_of(v: float) -> String:
	var a := absf(v)
	if a >= 1000.0:
		return "%.0f" % v
	if a >= 10.0:
		return "%.1f" % v
	if a >= 0.1 or a == 0.0:
		return "%.2f" % v
	return "%.2e" % v


func _draw() -> void:
	var font := _font()
	var fs := _font_size()
	var ink := _ink()
	var faint := Color(ink, ink.a * 0.25)
	# The legend: one line per series, wrapped across the width.
	var at := Vector2(PAD.x, fs + 2)
	if title != "":
		draw_string(font, Vector2(4, at.y), title, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, ink)
		at.y += LEGEND_LINE
	for s in series:
		var last = null
		for p in s["points"]:
			if p[1] != null:
				last = p[1]
		var text: String = "%s %s" % [s["name"], "NA" if last == null else label_of(float(last))]
		var w := font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, fs).x + 18
		if at.x + w > size.x and at.x > PAD.x:
			at = Vector2(PAD.x, at.y + LEGEND_LINE)
		draw_line(at + Vector2(0, -fs * 0.35), at + Vector2(10, -fs * 0.35), s["colour"], 3.0)
		draw_string(font, at + Vector2(14, 0), text, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, s["colour"])
		at.x += w + 12
	var top := at.y + 10
	var plot := Rect2(PAD.x, top, size.x - PAD.x - PAD.y, size.y - top - fs - 10)
	if plot.size.x < 20 or plot.size.y < 20:
		return
	draw_rect(plot, faint, false, 1.0)
	var b := bounds()
	if b.is_empty():
		draw_string(font, plot.position + Vector2(8, fs + 4), "no rows yet", HORIZONTAL_ALIGNMENT_LEFT, -1, fs, ink)
		return
	var to_px := func(x: float, y: float) -> Vector2:
		return Vector2(plot.position.x + (x - b[0]) / (b[1] - b[0]) * plot.size.x,
			plot.end.y - (y - b[2]) / (b[3] - b[2]) * plot.size.y)
	for k in TICKS + 1:
		var f := float(k) / TICKS
		var y: float = lerpf(b[2], b[3], f)
		var py: float = plot.end.y - f * plot.size.y
		draw_line(Vector2(plot.position.x, py), Vector2(plot.end.x, py), faint, 1.0)
		draw_string(font, Vector2(4, py + fs * 0.35), label_of(y), HORIZONTAL_ALIGNMENT_LEFT, PAD.x - 8, fs, ink)
	draw_string(font, Vector2(plot.position.x, plot.end.y + fs + 6), "minute %s" % label_of(b[0]), HORIZONTAL_ALIGNMENT_LEFT, -1, fs, ink)
	var right := "minute %s" % label_of(b[1])
	draw_string(font, Vector2(plot.end.x - font.get_string_size(right, HORIZONTAL_ALIGNMENT_LEFT, -1, fs).x, plot.end.y + fs + 6),
		right, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, ink)
	for s in series:
		var run := PackedVector2Array()
		for p in s["points"]:
			if p[1] == null:
				_stroke(run, s["colour"])
				run = PackedVector2Array()
				continue
			run.append(to_px.call(float(p[0]), float(p[1])))
		_stroke(run, s["colour"])


func _stroke(run: PackedVector2Array, colour: Color) -> void:
	if run.size() == 1:
		draw_circle(run[0], 2.5, colour)
	elif run.size() > 1:
		draw_polyline(run, colour, 2.0, true)
