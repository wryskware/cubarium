extends RefCounted
## Preserve the authored 1920×1080 layout, shrinking the whole UI only when needed.
## The logical canvas still covers the window, so anchored scrims and HUDs reach its edges.

const DESIGN_SIZE := Vector2(1920, 1080)


static func apply(canvas: Control, screen: Vector2) -> float:
	var fit := maxf(0.001, minf(1.0, minf(screen.x / DESIGN_SIZE.x, screen.y / DESIGN_SIZE.y)))
	var rescaled := not canvas.scale.is_equal_approx(Vector2.ONE * fit)
	canvas.set_anchors_and_offsets_preset(Control.PRESET_TOP_LEFT)
	canvas.position = Vector2.ZERO
	canvas.scale = Vector2.ONE * fit
	canvas.size = screen / fit
	if rescaled:
		# Gridlite text draws solid below scale 1 and dotted at 1 (NeonDotField, "Scaled
		# UI"); a new scale moves nothing, so redraw it all.
		canvas.propagate_call(&"queue_redraw")
	return fit


## The start-up window: the project's 1920×1080, shrunk to the screen's usable area where
## the screen is smaller (Wrysk's 2560×720 display: 1920×720), and centred there. Smaller
## windows then shrink the UI through apply(). Sized through the Window, not the
## DisplayServer, so the root viewport follows at once (on X11 without a window manager an
## in-place DisplayServer resize never reaches the viewport). Returns the window size.
static func fit_window(window: Window) -> Vector2i:
	if DisplayServer.get_name() == "headless" or window.mode != Window.MODE_WINDOWED:
		return window.size
	var usable := DisplayServer.screen_get_usable_rect(window.current_screen)
	if usable.size.x <= 0 or usable.size.y <= 0:
		return window.size
	var want := window.size.min(usable.size)
	if want != window.size:
		window.size = want
		if DisplayServer.get_name() != "Wayland":  # Wayland places windows itself
			window.position = usable.position + (usable.size - want) / 2
	return window.size


static func fit_inspector(panel: Control, canvas: Control) -> void:
	# Keep the inspector's authored 520×1000 shape alongside the centred page.
	panel.anchor_bottom = 0.0
	var top := maxf(0.0, (canvas.size.y - DESIGN_SIZE.y) * 0.5) + 40.0
	panel.offset_top = top
	panel.offset_bottom = top + 1000.0
