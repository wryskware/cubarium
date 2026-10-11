@tool
class_name NeonDotSlider
extends Control
## Round 4's steps (design/ui/design-language.md, "Settings controls (round 4)"): a track of
## 6 px square dots on a 9 px pitch, lit cyan up to the handle and faint beyond it; the
## handle a 12 × 36 px plate of #FFD6F6 dots on a 3 px grid inside a 3 px hot ring; the stop
## names under the track in chrome, the current one in ink. Left and right move between
## stops; a click or drag on it snaps to the nearest stop. Two stops (Smooth / Stepped) work
## the same way. Focus is the glow around the whole control. Every value below is an
## Inspector field.
##
## `index` set from code moves quietly; `step` and `choose` act as a viewer would and emit
## `changed`.

signal changed(index: int)

## The stops' names, left to right.
@export var stops := PackedStringArray(["Low", "Medium", "High"]):
	set(v):
		stops = v
		index = index
		update_minimum_size()
		queue_redraw()
## The current stop.
@export var index := 1:
	set(v):
		index = clampi(v, 0, maxi(stops.size() - 1, 0))
		queue_redraw()
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
## Space between the edge and the track and names, so the focus glow (drawn around the
## whole control) stands clear of them.
@export var padding := Vector2i(16, 12):
	set(v):
		padding = v
		update_minimum_size()
		queue_redraw()
## Drawn around the whole control while focused.
@export var focus_style: StyleBox = preload("res://ui/neon/styles/glow_focus.tres"):
	set(v):
		focus_style = v
		queue_redraw()
## Percent of its opacity the control keeps while disabled.
@export_range(0, 100, 1, "suffix:%") var disabled_opacity := 40:
	set(v):
		disabled_opacity = v
		_apply_disabled()

@export_group("Track")
## Space above the track.
@export_range(0, 100, 1, "suffix:px") var top_pad := 4:
	set(v):
		top_pad = v
		update_minimum_size()
		queue_redraw()
## The track's row: the dots centre in it and the handle fills it.
@export_range(1, 200, 1, "suffix:px") var track_height := 36:
	set(v):
		track_height = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var dot_size := 6:
	set(v):
		dot_size = v
		queue_redraw()
## Distance between dot origins.
@export_range(1, 64, 1, "suffix:px") var pitch := 9:
	set(v):
		pitch = v
		queue_redraw()
@export var lit_color := Color("#42C5F8"):
	set(v):
		lit_color = v
		queue_redraw()
@export var unlit_color := Color(165 / 255.0, 149 / 255.0, 216 / 255.0, 0.25):
	set(v):
		unlit_color = v
		queue_redraw()

@export_group("Handle")
@export var handle_size := Vector2i(12, 36):
	set(v):
		handle_size = v
		queue_redraw()
@export var handle_dot_color := Color("#FFD6F6"):
	set(v):
		handle_dot_color = v
		queue_redraw()
@export_range(1, 16, 1, "suffix:px") var handle_dot := 2:
	set(v):
		handle_dot = v
		queue_redraw()
@export_range(1, 16, 1, "suffix:px") var handle_pitch := 3:
	set(v):
		handle_pitch = v
		queue_redraw()
## The notched ring around the handle, drawn outside it.
@export var ring_color := Color("#F25BDB"):
	set(v):
		ring_color = v
		queue_redraw()
@export_range(0, 16, 1, "suffix:px") var ring_width := 3:
	set(v):
		ring_width = v
		queue_redraw()

@export_group("Stop names")
@export var stop_font: Font = preload("res://ui/neon/fonts/chrome.tres"):
	set(v):
		stop_font = v
		update_minimum_size()
		queue_redraw()
@export_range(1, 200, 1, "suffix:px") var stop_font_size := 26:
	set(v):
		stop_font_size = v
		update_minimum_size()
		queue_redraw()
@export var stop_color := Color("#A595D8"):
	set(v):
		stop_color = v
		queue_redraw()
@export var stop_on_color := Color("#F7F0FF"):
	set(v):
		stop_on_color = v
		queue_redraw()
@export var uppercase := true:
	set(v):
		uppercase = v
		queue_redraw()
## Between the track's row and the names.
@export_range(0, 100, 1, "suffix:px") var names_gap := 8:
	set(v):
		names_gap = v
		update_minimum_size()
		queue_redraw()


func _init() -> void:
	focus_mode = Control.FOCUS_ALL
	mouse_filter = Control.MOUSE_FILTER_STOP


func _ready() -> void:
	_apply_disabled()
	focus_entered.connect(queue_redraw)
	focus_exited.connect(queue_redraw)


## As a viewer would: one stop left (-1) or right (+1), stopping at the ends.
func step(delta: int) -> void:
	choose(index + delta)


## As a viewer would: stop `i`, reporting `changed` when it moves.
func choose(i: int) -> void:
	if disabled or stops.is_empty():
		return
	i = clampi(i, 0, stops.size() - 1)
	if i == index:
		return
	index = i
	changed.emit(index)


func value() -> String:
	return stops[index] if index >= 0 and index < stops.size() else ""


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
		choose(nearest_stop(event.position.x))
		accept_event()
	elif event is InputEventMouseMotion and event.button_mask & MOUSE_BUTTON_MASK_LEFT:
		choose(nearest_stop(event.position.x))
		accept_event()


## The stop whose dot is nearest to `x` (px from the control's left).
func nearest_stop(x: float) -> int:
	var best := 0
	for i in stops.size():
		if absf(_stop_x(i) - x) < absf(_stop_x(best) - x):
			best = i
	return best


## Width inside the padding.
func _inner_w() -> float:
	return maxf(0.0, size.x - 2 * padding.x)


func _dots() -> int:
	return maxi(1, int(floor((_inner_w() + pitch - dot_size) / float(pitch))))


## The dot the handle sits on at stop `i`.
func _stop_dot(i: int) -> int:
	var k := stops.size()
	return 0 if k <= 1 else int(round(float(i) / (k - 1) * (_dots() - 1)))


func _stop_x(i: int) -> float:
	return padding.x + _stop_dot(i) * pitch + dot_size / 2.0


func _apply_disabled() -> void:
	modulate.a = disabled_opacity / 100.0 if disabled else 1.0
	focus_mode = Control.FOCUS_NONE if disabled else Control.FOCUS_ALL
	if disabled and is_inside_tree() and has_focus():
		release_focus()
	queue_redraw()


func _get_minimum_size() -> Vector2:
	var h := float(top_pad + track_height)
	var w := 0.0
	if stop_font != null and not stops.is_empty():
		h += names_gap + stop_font.get_height(stop_font_size)
		for s in stops:
			w += stop_font.get_string_size(s.to_upper() if uppercase else s, HORIZONTAL_ALIGNMENT_LEFT, -1, stop_font_size).x
		w += 24.0 * (stops.size() - 1)
	return Vector2(w, h) + 2 * Vector2(padding)


func _draw() -> void:
	draw_set_transform(Vector2(padding))
	var n := _dots()
	var at := _stop_dot(index)
	var dy := top_pad + floorf((track_height - dot_size) / 2.0)
	for i in n:
		draw_rect(Rect2(i * pitch, dy, dot_size, dot_size), lit_color if i <= at else unlit_color)
	# The handle: a dot plate in a notched ring, centred on its dot.
	var hx := roundf(at * pitch + dot_size / 2.0 - handle_size.x / 2.0)
	var hr := Rect2(hx, top_pad, handle_size.x, handle_size.y)
	var y := 0
	while y < handle_size.y:
		var x := 0
		while x < handle_size.x:
			draw_rect(Rect2(hr.position + Vector2(x, y), Vector2(mini(handle_dot, handle_size.x - x), mini(handle_dot, handle_size.y - y))), handle_dot_color)
			x += handle_pitch
		y += handle_pitch
	if ring_width > 0:
		var w := float(ring_width)
		draw_rect(Rect2(hr.position.x, hr.position.y - w, hr.size.x, w), ring_color)
		draw_rect(Rect2(hr.position.x, hr.end.y, hr.size.x, w), ring_color)
		draw_rect(Rect2(hr.position.x - w, hr.position.y, w, hr.size.y), ring_color)
		draw_rect(Rect2(hr.end.x, hr.position.y, w, hr.size.y), ring_color)
	# The names, spread as CSS's space-between: first at the left, last at the right.
	if stop_font != null and not stops.is_empty():
		var names: Array[String] = []
		var widths: Array[float] = []
		var total := 0.0
		for s in stops:
			var t: String = s.to_upper() if uppercase else s
			names.append(t)
			var tw := stop_font.get_string_size(t, HORIZONTAL_ALIGNMENT_LEFT, -1, stop_font_size).x
			widths.append(tw)
			total += tw
		var spare := _inner_w() - total
		var between := spare / (names.size() - 1) if names.size() > 1 else 0.0
		var baseline := roundf(top_pad + track_height + names_gap + stop_font.get_ascent(stop_font_size))
		var x := 0.0
		for i in names.size():
			draw_string(stop_font, Vector2(roundf(x), baseline), names[i], HORIZONTAL_ALIGNMENT_LEFT, -1,
					stop_font_size, stop_on_color if i == index else stop_color)
			NeonDotField.draw_string_field(self, stop_font, Vector2(roundf(x), baseline), names[i],
					HORIZONTAL_ALIGNMENT_LEFT, -1, stop_font_size, stop_on_color if i == index else stop_color)
			x += widths[i] + between
	draw_set_transform(Vector2.ZERO)
	if (has_focus() or preview_focus) and not disabled and focus_style != null:
		draw_style_box(focus_style, Rect2(Vector2.ZERO, size))
