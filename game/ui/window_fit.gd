extends RefCounted
## Preserve the authored 1920×1080 layout, shrinking the whole UI only when needed.
## The logical canvas still covers the window, so anchored scrims and HUDs reach its edges.

const DESIGN_SIZE := Vector2(1920, 1080)


static func apply(canvas: Control, screen: Vector2) -> float:
	var fit := maxf(0.001, minf(1.0, minf(screen.x / DESIGN_SIZE.x, screen.y / DESIGN_SIZE.y)))
	canvas.set_anchors_and_offsets_preset(Control.PRESET_TOP_LEFT)
	canvas.position = Vector2.ZERO
	canvas.scale = Vector2.ONE * fit
	canvas.size = screen / fit
	return fit


static func fit_inspector(panel: Control, canvas: Control) -> void:
	# Keep the inspector's authored 520×1000 shape alongside the centred page.
	panel.anchor_bottom = 0.0
	var top := maxf(0.0, (canvas.size.y - DESIGN_SIZE.y) * 0.5) + 40.0
	panel.offset_top = top
	panel.offset_bottom = top + 1000.0
