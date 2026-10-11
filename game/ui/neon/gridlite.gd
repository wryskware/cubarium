class_name Gridlite
extends RefCounted
## Gridlite PE's dot geometry, read from the font's own variable outlines, for drawing its
## unlit dots ourselves (NeonDotField; chrome-dots, 2026-10-11).
##
## What the outlines say (measured from GridlitePEVFTRIAL-All.ttf):
## - Every glyph is 7 dots wide (2800 of 5200 units) and 13 dots tall: centres on a 400-unit
##   lattice from the pen position and the baseline, columns 0..6, rows -3..9 (y -1200..3600).
## - Each of those 91 cells holds exactly one dot, which belongs to one zone: lit (`wght`),
##   BACK or RECT. The zones never overlap, so nothing is combined.
##   - BACK is a band of whole rows across the glyph's full advance, minus its lit cells:
##     rows 0..6 for capitals and figures, 0..4 for x-height letters, -2..4 or -2..6 with a
##     descender, and so on per glyph.
##   - RECT is every other cell of the line box. A space is all RECT. Letter spacing added
##     by the renderer (FontVariation.spacing_glyph) has no dots at all.
## - A dot's side is 400 × (v - 1) / 899 units for axis value v (1 to 900) in every zone,
##   so nothing shows at 1 and a dot fills its cell at 900. ELSH shapes the unlit dots
##   as well as the lit ones (2.65: taller than wide; 3.5: rounded squares).
## The theme's Gridlite fonts set BACK and RECT to 1 (no dots of their own) and carry the
## role's values as metadata `dot_back` and `dot_rect`: see field_spec().

const UPM := 5200.0
## Units between dot centres.
const PITCH := 400.0
const COLUMNS := 7
const ROW_LOW := -3
const ROWS := 13

enum Zone { RECT, BACK, LIT }

## Size in px the outlines are read at: 40 px between dot centres.
const _PROBE_SIZE := 520

static var _probes := {}
static var _cells := {}
static var _areas := {}


## The drawn field this font asks for, or an empty Dictionary when it has none: a
## FontVariation whose own BACK and RECT are off and whose metadata names the zones'
## values. Keys: back, rect (axis values 1..900), elsh, uniform (an alpha: one flat 1 px
## field of that alpha instead of the two zones, for the chrome-dots trial's variant B).
static func field_spec(font: Font) -> Dictionary:
	var fv := font as FontVariation
	if fv == null or fv.base_font == null:
		return {}
	if not (fv.has_meta(&"dot_back") or fv.has_meta(&"dot_rect") or fv.has_meta(&"dot_uniform")):
		return {}
	return {"back": float(fv.get_meta(&"dot_back", 1.0)), "rect": float(fv.get_meta(&"dot_rect", 1.0)),
		"elsh": axis(fv, "ELSH", 3.0), "uniform": float(fv.get_meta(&"dot_uniform", -1.0))}


static func tag(axis_name: String) -> int:
	return TextServerManager.get_primary_interface().name_to_tag(axis_name)


## The font's value for an axis, or `default` when it doesn't set one.
static func axis(font: FontVariation, axis_name: String, default: float) -> float:
	return float(font.variation_opentype.get(tag(axis_name), default))


## The px between dot centres at font size `size`.
static func pitch_px(size: int) -> float:
	return PITCH * size / UPM


## The zone of every cell of a glyph, row-major from row ROW_LOW up, COLUMNS per row.
static func cells(base_font: Font, glyph: int) -> PackedByteArray:
	var key := "%d:%d" % [base_font.get_instance_id(), glyph]
	if _cells.has(key):
		return _cells[key]
	var res := PackedByteArray()
	res.resize(ROWS * COLUMNS)
	res.fill(Zone.RECT)
	# lit dots fill their cell (40 px here), BACK dots half of it, RECT dots nothing
	var probe := _probe(base_font, {"wght": 900.0, "BACK": 450.0, "RECT": 1.0, "ELSH": 3.0})
	var ts := TextServerManager.get_primary_interface()
	var c: Dictionary = ts.font_get_glyph_contours(probe.get_rids()[0], _PROBE_SIZE, glyph)
	var p := pitch_px(_PROBE_SIZE)
	for b in _contour_bounds(c):
		var side := maxf(b.size.x, b.size.y)
		if side < p * 0.25:
			continue
		var col := int(roundf(b.get_center().x / p))
		var row := int(roundf(-b.get_center().y / p))
		if col < 0 or col >= COLUMNS or row < ROW_LOW or row >= ROW_LOW + ROWS:
			continue
		res[(row - ROW_LOW) * COLUMNS + col] = Zone.LIT if side > p * 0.75 else Zone.BACK
	_cells[key] = res
	return res


## The area of one dot in font units² at axis value `v` and dot shape `elsh`: what a dot
## of any zone covers, read from a space's outline (a space is all RECT).
static func dot_area(base_font: Font, v: float, elsh: float) -> float:
	if v <= 1.0:
		return 0.0
	var key := "%d:%.3f:%.3f" % [base_font.get_instance_id(), v, elsh]
	if _areas.has(key):
		return _areas[key]
	var probe := _probe(base_font, {"wght": 1.0, "BACK": 1.0, "RECT": v, "ELSH": elsh})
	var ts := TextServerManager.get_primary_interface()
	var rid: RID = probe.get_rids()[0]
	var space := ts.font_get_glyph_index(rid, _PROBE_SIZE, 32, 0)
	var c: Dictionary = ts.font_get_glyph_contours(rid, _PROBE_SIZE, space)
	var pts: PackedVector3Array = c.get("points", PackedVector3Array())
	var ends: PackedInt32Array = c.get("contours", PackedInt32Array())
	var area := 0.0
	if not ends.is_empty():
		area = absf(_contour_area(pts, 0, ends[0]))
	var scale := UPM / _PROBE_SIZE
	area *= scale * scale
	_areas[key] = area
	return area


## The light one dot carries at font size `size`, in px² of full colour: the dot's area.
## Godot's render of the font's own dots matches this within 1 % at 26 px and 5 % at 91 px
## (tests/gridlite_light.gd).
static func dot_light(base_font: Font, v: float, elsh: float, size: int) -> float:
	var s := size / UPM
	return dot_area(base_font, v, elsh) * s * s


## A whole-pixel dot carrying `light` px²: its side n (whole px, 1 up to the pitch less
## `min_gap`) and alpha (light / n², at most 1). n is the side of a square of that area,
## rounded; where even the largest allowed n can't carry the light at alpha 1, the dot
## comes out dimmer than the font's, short of it by `short` px².
static func whole_dot(light: float, pitch: int, min_gap: int) -> Dictionary:
	var most := maxi(1, pitch - min_gap)
	var n := clampi(int(roundf(sqrt(light))), 1, most)
	var a := light / float(n * n)
	return {"size": n, "alpha": minf(a, 1.0), "short": maxf(0.0, a - 1.0) * n * n}


static func _probe(base_font: Font, axes: Dictionary) -> FontVariation:
	var key := "%d:%s" % [base_font.get_instance_id(), str(axes)]
	if _probes.has(key):
		return _probes[key]
	var fv := FontVariation.new()
	fv.base_font = base_font
	var ot := {}
	for a in axes:
		ot[tag(a)] = axes[a]
	fv.variation_opentype = ot
	_probes[key] = fv
	return fv


static func _contour_bounds(c: Dictionary) -> Array[Rect2]:
	var res: Array[Rect2] = []
	var pts: PackedVector3Array = c.get("points", PackedVector3Array())
	var start := 0
	for e in c.get("contours", PackedInt32Array()):
		if e >= start:
			var r := Rect2(pts[start].x, pts[start].y, 0, 0)
			for i in range(start + 1, e + 1):
				r = r.expand(Vector2(pts[i].x, pts[i].y))
			res.append(r)
		start = e + 1
	return res


## The signed area of one TrueType contour (points[from..to]; z 1 on the curve, 0 a
## quadratic control point, with implied on-curve midpoints between two controls).
static func _contour_area(pts: PackedVector3Array, from: int, to: int) -> float:
	var n := to - from + 1
	if n < 3:
		return 0.0
	# start from an on-curve point
	var s := -1
	for i in n:
		if pts[from + i].z != 0.0:
			s = i
			break
	if s < 0:
		return 0.0
	var prev := Vector2(pts[from + s].x, pts[from + s].y)
	var area := 0.0
	var pending = null
	for k in range(1, n + 1):
		var q := pts[from + (s + k) % n]
		var v := Vector2(q.x, q.y)
		if q.z == 0.0:
			if pending != null:
				var mid: Vector2 = (pending + v) / 2.0
				area += _segment_area(prev, pending, mid)
				prev = mid
			pending = v
		else:
			area += _segment_area(prev, pending, v)
			prev = v
			pending = null
	return area


## ∮ contribution of one segment: the line's shoelace term, plus two thirds of the control
## triangle for a quadratic.
static func _segment_area(a: Vector2, c, b: Vector2) -> float:
	var t := (a.x * b.y - b.x * a.y) / 2.0
	if c != null:
		var cv: Vector2 = c
		t += (2.0 / 3.0) * ((a.x * cv.y - cv.x * a.y) + (cv.x * b.y - b.x * cv.y) + (b.x * a.y - a.x * b.y)) / 2.0
	return t
