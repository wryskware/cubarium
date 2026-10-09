@tool
class_name StyleBoxNeonFrame
extends StyleBox
## The neon UI's notched frame (design/ui/design-language.md, "Panels"): an optional fill, a
## ring with its corners cut drawn outside the rect, and an optional soft glow outside it.
## The main panel, inner boxes (plate, live count, search, tab bar) and the secondary button
## use it; StyleBoxNeonFocus is the same frame set up as the focus glow.
##
## The ring is four rectangles, one per side, so the corner squares stay empty (round 3's
## four offset box-shadows). The glow is round 3's CSS box-shadow: a Gaussian blur of the
## (outset + spread) rect, drawn only outside the outset rect.

## Flat colour under everything inside the rect. Transparent draws nothing.
@export var fill_color := Color(0, 0, 0, 0):
	set(v):
		fill_color = v
		emit_changed()
## Another style drawn inside the rect after fill_color, e.g. the dithered bands.
@export var fill_style: StyleBox:
	set(v):
		if fill_style != null and fill_style.changed.is_connected(emit_changed):
			fill_style.changed.disconnect(emit_changed)
		fill_style = v
		if fill_style != null:
			fill_style.changed.connect(emit_changed)
		emit_changed()

@export_group("Ring")
## The notched line's colour. Transparent draws no ring.
@export var ring_color := Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.32):
	set(v):
		ring_color = v
		emit_changed()
## The line's thickness in px, drawn outside the (outset) rect.
@export_range(0, 24, 1, "suffix:px") var ring_width := 3:
	set(v):
		ring_width = v
		emit_changed()
## Moves the ring and glow out from the control's rect by this many px (the focus glow
## sits 6 px out). The fill always covers the control's own rect.
@export_range(-64, 64, 1, "suffix:px") var outset := 0:
	set(v):
		outset = v
		emit_changed()

@export_group("Glow")
## The glow's colour and strength. Transparent draws no glow.
@export var glow_color := Color(0, 0, 0, 0):
	set(v):
		glow_color = v
		emit_changed()
## Blur radius in px, as CSS box-shadow's (a Gaussian with sigma = half of it).
@export_range(0, 200, 1, "suffix:px") var glow_blur := 0:
	set(v):
		glow_blur = v
		emit_changed()
## Grows the glowing shape by this many px before blurring, as CSS box-shadow's spread.
@export_range(0, 64, 1, "suffix:px") var glow_spread := 0:
	set(v):
		glow_spread = v
		emit_changed()

# Glow nine-patch textures by blur and spread, shared by every frame.
static var _glow_cache := {}


func _draw(to_canvas_item: RID, rect: Rect2) -> void:
	var rs := RenderingServer
	var r := rect.grow(outset)
	if glow_color.a > 0.0 and (glow_blur > 0 or glow_spread > 0):
		var tex := _glow_texture(glow_blur, glow_spread)
		var m := (tex.get_width() - 2) / 2
		rs.canvas_item_add_nine_patch(to_canvas_item, r.grow(m), Rect2(Vector2.ZERO, tex.get_size()),
				tex.get_rid(), Vector2(m, m), Vector2(m, m), RenderingServer.NINE_PATCH_STRETCH,
				RenderingServer.NINE_PATCH_STRETCH, false, glow_color)
	if fill_color.a > 0.0:
		rs.canvas_item_add_rect(to_canvas_item, rect, fill_color)
	if fill_style != null:
		fill_style.draw(to_canvas_item, rect)
	if ring_color.a > 0.0 and ring_width > 0:
		var w := float(ring_width)
		rs.canvas_item_add_rect(to_canvas_item, Rect2(r.position.x, r.position.y - w, r.size.x, w), ring_color)
		rs.canvas_item_add_rect(to_canvas_item, Rect2(r.position.x, r.end.y, r.size.x, w), ring_color)
		rs.canvas_item_add_rect(to_canvas_item, Rect2(r.position.x - w, r.position.y, w, r.size.y), ring_color)
		rs.canvas_item_add_rect(to_canvas_item, Rect2(r.end.x, r.position.y, w, r.size.y), ring_color)


func _get_draw_rect(rect: Rect2) -> Rect2:
	var reach := maxi(ring_width, 0)
	if glow_color.a > 0.0:
		reach = maxi(reach, glow_spread + ceili(glow_blur * 1.5))
	return rect.grow(outset + reach)


## A white nine-patch whose alpha is the blurred shape outside the clip edge: m texels out
## on each side, two texels across the middle that stretch along the edges.
static func _glow_texture(blur: int, spread: int) -> ImageTexture:
	var key := Vector2i(blur, spread)
	if _glow_cache.has(key):
		return _glow_cache[key]
	var sigma := maxf(blur / 2.0, 0.001)
	var m := spread + ceili(sigma * 3.0)
	var n := 2 * m + 2
	var profile := PackedFloat32Array()
	profile.resize(n)
	for i in n:
		var d: float # px outside the clip edge, at the texel's centre
		if i < m:
			d = m - i - 0.5
		elif i >= m + 2:
			d = i - (m + 2) + 0.5
		else:
			profile[i] = 1.0
			continue
		profile[i] = 0.5 * _erfc((d - spread) / (sigma * sqrt(2.0)))
	var img := Image.create_empty(n, n, false, Image.FORMAT_RGBA8)
	for y in n:
		for x in n:
			var a := profile[x] * profile[y]
			if x >= m and x < m + 2 and y >= m and y < m + 2:
				a = 0.0
			img.set_pixel(x, y, Color(1, 1, 1, a))
	var tex := ImageTexture.create_from_image(img)
	_glow_cache[key] = tex
	return tex


static func _erfc(x: float) -> float:
	# Abramowitz and Stegun 7.1.26, |error| < 1.5e-7
	var z := absf(x)
	var t := 1.0 / (1.0 + 0.3275911 * z)
	var y := t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))))
	var e := y * exp(-z * z)
	return e if x >= 0.0 else 2.0 - e
