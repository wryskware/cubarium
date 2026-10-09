@tool
class_name NeonBarChart
extends Control
## The live count's bar chart (round 3's field guide): one bar per value, right-aligned,
## bars 3 to 6 px wide with 3 px gaps; every bar at bar_alpha except the newest. A bar's
## height is (value - baseline) * px_per_unit. The next package feeds it live values.

@export var values := PackedFloat32Array([4, 4, 5, 5, 5, 6, 6, 6, 5, 5, 6, 7, 7, 7, 8, 8, 7, 7, 7,
		6, 6, 7, 7, 8, 8, 8, 7, 7, 7, 7, 8, 7, 7, 7, 7, 7]):
	set(v):
		values = v
		queue_redraw()
@export var color := Color("#42C5F8"):
	set(v):
		color = v
		queue_redraw()
@export_range(0.0, 1.0) var bar_alpha := 0.55:
	set(v):
		bar_alpha = v
		queue_redraw()
## The newest (last) bar's alpha.
@export_range(0.0, 1.0) var last_alpha := 1.0:
	set(v):
		last_alpha = v
		queue_redraw()
@export_range(0, 16, 1, "suffix:px") var gap := 3:
	set(v):
		gap = v
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var min_bar := 3:
	set(v):
		min_bar = v
		queue_redraw()
@export_range(1, 32, 1, "suffix:px") var max_bar := 6:
	set(v):
		max_bar = v
		queue_redraw()
@export var baseline := 2.0:
	set(v):
		baseline = v
		queue_redraw()
@export var px_per_unit := 8.0:
	set(v):
		px_per_unit = v
		queue_redraw()


func _draw() -> void:
	var n := values.size()
	if n == 0:
		return
	# bars share the width evenly; edges snap to whole pixels, as the browser lays them out
	var bw := clampf((size.x - gap * (n - 1)) / n, min_bar, max_bar)
	var x0 := size.x - (n * bw + (n - 1) * gap)
	for i in n:
		var a := roundf(x0 + i * (bw + gap))
		var b := roundf(x0 + i * (bw + gap) + bw)
		var h := clampf((values[i] - baseline) * px_per_unit, 0.0, size.y)
		draw_rect(Rect2(a, size.y - h, b - a, h), Color(color, color.a * (last_alpha if i == n - 1 else bar_alpha)))
