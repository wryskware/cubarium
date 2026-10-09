@tool
class_name StyleBoxNeonBands
extends StyleBox
## The neon UI's panel fill (design/ui/design-language.md, "Panels"): dithered bands. Five
## violet steps, darker towards the bottom, each edge blended across +-blend px with a 4x4
## Bayer ordered dither drawn in cell px squares. The picture is made once per size (a
## texture per rect size, kept until a value changes) and drawn at 1:1.

## Band colours, top to bottom.
@export var colors := PackedColorArray([Color("#2A1470"), Color("#24115F"), Color("#1D0D50"),
		Color("#160A41"), Color("#100632")]):
	set(v):
		colors = v
		_reset()
## Where each band ends, as a fraction of the height; one fewer than the colours (the last
## band runs to the bottom).
@export var edges := PackedFloat32Array([0.18, 0.36, 0.56, 0.76]):
	set(v):
		edges = v
		_reset()
## The dither's square size in px.
@export_range(1, 16, 1, "suffix:px") var cell := 3:
	set(v):
		cell = v
		_reset()
## How far each side of an edge the dither blends, in px (whole cells).
@export_range(0, 120, 1, "suffix:px") var blend := 15:
	set(v):
		blend = v
		_reset()

const BAYER := [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5]

var _cache := {}


func _reset() -> void:
	_cache.clear()
	emit_changed()


func _draw(to_canvas_item: RID, rect: Rect2) -> void:
	if colors.is_empty() or rect.size.x < 1.0 or rect.size.y < 1.0:
		return
	var tex := _texture(Vector2i(ceili(rect.size.x), ceili(rect.size.y)))
	RenderingServer.canvas_item_add_texture_rect_region(to_canvas_item, rect, tex.get_rid(),
			Rect2(Vector2.ZERO, rect.size))


func _texture(px: Vector2i) -> ImageTexture:
	if _cache.has(px):
		return _cache[px]
	if _cache.size() > 32:
		_cache.clear()
	var c := maxi(cell, 1)
	var cw := maxi(1, ceili(px.x / float(c)))
	var ch := maxi(1, ceili(px.y / float(c)))
	var n := colors.size()
	var ends := PackedInt32Array()
	for i in n:
		ends.append(roundi((edges[i] if i < edges.size() else 1.0) * ch) if i < n - 1 else ch)
	var z := roundi(blend / float(c))
	var img := Image.create_empty(cw, ch, false, Image.FORMAT_RGBA8)
	for y in ch:
		var b := n - 1
		for i in n - 1:
			if (y + 0.5) / ch < (edges[i] if i < edges.size() else 1.0):
				b = i
				break
		var eb := ends[b]
		var ea := ends[b - 1] if b > 0 else -1000000
		var near_below := z > 0 and b < n - 1 and eb - y <= z
		var near_above := z > 0 and b > 0 and y - ea < z
		if not near_below and not near_above:
			img.fill_rect(Rect2i(0, y, cw, 1), colors[b])
			continue
		for x in cw:
			var col := b
			var th: float = (BAYER[(y % 4) * 4 + (x % 4)] + 0.5) / 16.0
			# the neighbour band shows where its ramp beats the Bayer threshold
			if near_below and (z - (eb - y) + 0.5) / (2.0 * z) > th:
				col = b + 1
			if near_above and (z - (y - ea) - 0.5) / (2.0 * z) > th:
				col = b - 1
			img.set_pixel(x, y, colors[col])
	img.resize(cw * c, ch * c, Image.INTERPOLATE_NEAREST)
	var tex := ImageTexture.create_from_image(img)
	_cache[px] = tex
	return tex
