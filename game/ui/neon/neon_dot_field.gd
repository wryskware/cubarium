@tool
class_name NeonDotField
extends Control
## Unlit dots drawn as rects on whole device pixels, so they can't beat against the screen
## the way a font's own sub-pixel unlit dots do (chrome-dots, 2026-10-11).
##
## Label mode (`follow_label`, what the game uses): the field belongs to the Label it sits
## in and redraws whenever that label does. When the label's font is a Gridlite font with a
## drawn field (Gridlite.field_spec: the font's own BACK and RECT off, the role's values in
## its metadata), it draws Gridlite's two unlit zones cell for cell where the font would:
## BACK (the rows behind each letter) and RECT (the rest of the line box). Each zone's axis
## value becomes one whole-pixel dot per cell carrying the light the font's own dot carries
## today (Gridlite.dot_light, Gridlite.whole_dot), in the label's font colour, or in its
## shadow colour at the shadow offset for the SHADOW layer. The lit dots stay the font's.
## NeonDots (neon_dots.gd, an autoload) gives every Label two of these: the shadow layer
## behind the label and the text layer in front of it. They never cover a lit dot: each
## cell holds one zone only.
##
## The knobs are theme constants looked up on the label (its type variation, then Label):
## - dot_light: percent of the font's light each drawn dot carries (100).
## - dot_min_gap: px kept between neighbouring dots; a dot's side is at most the pitch less
##   this (0: at a 2 px pitch a dot may fill its cell, so a plate turns solid).
## - dot_min_gap_pressed, dot_min_gap_disabled: dot_min_gap while the NeonButton the label
##   sits in is pressed (held or toggled on) or disabled; dot_min_gap where unset. They let
##   one label (the main button's keycap) be a solid plate at rest and dotted pressed or
##   disabled (Wrysk's picks, 2026-10-11).
## - dot_gap_fill: percent of a dot's light (0-100) that the rest of its pitch x pitch cell
##   (the gap pixels around it) carries, in the same colour, where dot_min_gap leaves a gap.
##   It fills plate cells only (the drawn zones), never the letters' cells, so a dotted
##   negative plate reads as full-strength dots over a fainter plate with dark letters cut
##   out (Wrysk's in-between pick, 2026-10-11). 0: gaps stay empty.
## - dot_gap_fill_pressed, dot_gap_fill_disabled: dot_gap_fill while the label's button is
##   pressed or disabled, as dot_min_gap_pressed / _disabled; dot_gap_fill where unset.
## - dot_fill_spacing: 1 also fills the letter-spacing columns (spacing_glyph) with RECT
##   dots; 0 leaves them empty, as the font does.
##
## - dot_scaled_keep_pitch: see "Scaled UI" below; 0 (never keep the dots when scaled).
##
## Scaled UI (ui-scale, 2026-10-11; Wrysk: "when we scale, we default back to solid"):
## whenever the label (or the control drawing text) is drawn at a scale other than 1
## (WindowFit shrinks the whole UI on small windows), the dots' pitch turns fractional and
## the plates and the font's own lit dots beat into stripes. So it is drawn solid instead:
## every cell of each zone (BACK, RECT, and the lit letters) is filled flat, carrying the
## same light per area as its dotted cell (the dot, plus its gap fill; for the letters the
## font's lit dot at its wght), and the label's own glyphs are hidden (self_modulate alpha
## 0) so only the solid cells show. Letters become solid pixel strokes on the cell grid; a
## negative label stays a plate with its letters cut out. dot_scaled_keep_pitch (px on
## screen) keeps a role dotted when scaled as long as its on-screen pitch is at least that
## (e.g. 4 keeps the 91 px title dotted at 0.667, where its pitch is 4.67 px). At scale 1
## nothing changes.
##
## Uniform mode (follow_label off; the chrome-dots trial, tests/font_trial_shots.gd): a
## `dot_size` square of `color` at `first` + k × `pitch` across the control's rect.

enum Layer { TEXT, SHADOW }

const LIGHT_DEFAULT := 100
const MIN_GAP_DEFAULT := 0
const FILL_SPACING_DEFAULT := 0
const GAP_FILL_DEFAULT := 0
const SCALED_KEEP_PITCH_DEFAULT := 0
## The label's own glyphs are hidden by us (solid when scaled).
const _HIDDEN := &"_neon_solid_hidden"

@export var color := Color(0.97, 0.94, 1.0, 0.5):
	set(v):
		color = v
		queue_redraw()
## Each dot's side in px.
@export_range(1, 16, 1, "suffix:px") var dot_size := 1:
	set(v):
		dot_size = v
		queue_redraw()
## Distance between dot centres in px (the font's dot pitch at its size).
@export_range(1, 64, 1, "suffix:px") var pitch := 2:
	set(v):
		pitch = v
		queue_redraw()
## The first dot's centre in the control's own px; may be fractional (a font's dot centre
## on a pixel corner is a whole number, on a pixel centre a half).
@export var first := Vector2(1, 1):
	set(v):
		first = v
		queue_redraw()
## Draw the parent Label's Gridlite zones instead of a uniform field.
@export var follow_label := false:
	set(v):
		follow_label = v
		_bind()
		queue_redraw()
## TEXT draws in the label's font colour; SHADOW in its shadow colour at its shadow offset.
@export var layer := Layer.TEXT:
	set(v):
		layer = v
		queue_redraw()

var _label: Label


func _enter_tree() -> void:
	_bind()


func _exit_tree() -> void:
	_unbind()


func _bind() -> void:
	_unbind()
	if not follow_label or not is_inside_tree():
		return
	_label = get_parent() as Label
	if _label != null:
		_label.draw.connect(queue_redraw)
		mouse_filter = Control.MOUSE_FILTER_IGNORE


func _unbind() -> void:
	if _label != null and is_instance_valid(_label) and _label.draw.is_connected(queue_redraw):
		_label.draw.disconnect(queue_redraw)
	_label = null


func _draw() -> void:
	if follow_label:
		if _label != null:
			_draw_label(_label)
		return
	if color.a <= 0.0 or pitch <= 0:
		return
	var o := get_global_transform_with_canvas().origin
	var half := dot_size / 2.0
	var y := first.y
	while y < size.y:
		var x := first.x
		while x < size.x:
			# snap the dot's top-left to a whole canvas pixel (centre on a corner: the
			# pixel right and below it)
			var g := (o + Vector2(x, y) - Vector2(half, half) + Vector2(0.501, 0.501)).floor()
			draw_rect(Rect2(g - o, Vector2(dot_size, dot_size)), color)
			x += pitch
		y += pitch


func _draw_label(l: Label) -> void:
	var font := l.get_theme_font("font")
	var spec := Gridlite.field_spec(font)
	var font_size := l.get_theme_font_size("font_size")
	var knobs := _knobs(l, true, _button_state(l))
	var solid := not spec.is_empty() and solid_at(l, font_size, knobs)
	if layer == Layer.TEXT:
		_hide_glyphs(l, solid)
	if not l.is_visible_in_tree() or l.text == "" or spec.is_empty():
		return
	var c := l.get_theme_color("font_color")
	var off := Vector2.ZERO
	if layer == Layer.SHADOW:
		c = l.get_theme_color("font_shadow_color")
		off = Vector2(l.get_theme_constant("shadow_offset_x"), l.get_theme_constant("shadow_offset_y"))
	if c.a <= 0.0:
		return
	var text := l.text.to_upper() if l.uppercase else l.text
	var glyphs := _glyphs(font, font_size, text)
	# each glyph's pen position from the label's own layout (alignment, wrapping)
	var asc := font.get_ascent(font_size)
	for g in glyphs:
		var b := l.get_character_bounds(g.start)
		if b.size == Vector2.ZERO:
			g.index = -1  # not shown (clipped, trimmed or past visible_characters)
			g.rid = RID()
		var pen: Vector2 = Vector2(b.position.x, b.position.y + asc) + g.offset
		g.pen = pen if solid else _pen(pen, l)
	if solid:
		draw_glyph_cells_solid(self, font, font_size, glyphs, c, off, knobs)
	else:
		draw_glyph_zones(self, font, font_size, glyphs, c, off, knobs)


## Whether `ci` draws Gridlite text at `font_size` solid (see "Scaled UI" above): it is
## drawn at a scale other than 1, and the role doesn't keep its dots there.
static func solid_at(ci: CanvasItem, font_size: int, knobs: Dictionary) -> bool:
	var s := ci.get_global_transform_with_canvas().get_scale().abs()
	if absf(s.x - 1.0) <= 0.001 and absf(s.y - 1.0) <= 0.001:
		return false
	var keep: int = knobs.get("keep_pitch", SCALED_KEEP_PITCH_DEFAULT)
	return keep <= 0 or Gridlite.pitch_px(font_size) * minf(s.x, s.y) < keep


## Whether draw_string_field() draws the letters too (solid), so the caller must not
## draw_string() them itself.
static func draws_letters(ci: CanvasItem, font: Font, font_size: int) -> bool:
	if Gridlite.field_spec(font).is_empty():
		return false
	return solid_at(ci, font_size, _knobs(ci as Control, false) if ci is Control else _knobs(null, false))


## Hides (or shows again) the label's own glyphs while its field draws them solid. Only a
## label this hid is ever shown again, so at scale 1 nothing is touched.
static func _hide_glyphs(l: Label, hide: bool) -> void:
	if hide:
		l.set_meta(_HIDDEN, true)
		l.self_modulate = Color(1, 1, 1, 0)
	elif l.has_meta(_HIDDEN):
		l.remove_meta(_HIDDEN)
		l.self_modulate = Color.WHITE


## The drawn Gridlite field for text a control draws itself with draw_string(): call it
## from that control's _draw() with draw_string's own arguments. Draws nothing for a font
## without a drawn field.
static func draw_string_field(ci: CanvasItem, font: Font, pos: Vector2, text: String,
		alignment := HORIZONTAL_ALIGNMENT_LEFT, width := -1.0, font_size := 16,
		modulate := Color.WHITE) -> void:
	if Gridlite.field_spec(font).is_empty() or text == "":
		return
	var knobs := _knobs(ci as Control, false) if ci is Control else _knobs(null, false)
	var solid := solid_at(ci, font_size, knobs)
	var glyphs := _glyphs(font, font_size, text)
	var w := 0.0
	for g in glyphs:
		w += g.advance
	var x := pos.x
	if width > 0.0:
		match alignment:
			HORIZONTAL_ALIGNMENT_CENTER: x += floorf((width - w) / 2.0)
			HORIZONTAL_ALIGNMENT_RIGHT: x += width - w
	for g in glyphs:
		g.pen = Vector2(x, pos.y) + g.offset if solid else _pen(Vector2(x, pos.y) + g.offset, ci)
		x += g.advance
	if solid:
		draw_glyph_cells_solid(ci, font, font_size, glyphs, modulate, Vector2.ZERO, knobs)
	else:
		draw_glyph_zones(ci, font, font_size, glyphs, modulate, Vector2.ZERO, knobs)


## Draws the zones behind `glyphs` (from _glyphs, each with its `pen`: the pen position on
## the baseline, in `ci`'s px) in colour `c`, shifted by `off`.
static func draw_glyph_zones(ci: CanvasItem, font: Font, font_size: int, glyphs: Array,
		c: Color, off: Vector2, knobs: Dictionary) -> void:
	var spec := Gridlite.field_spec(font)
	var base: Font = (font as FontVariation).base_font
	var p := Gridlite.pitch_px(font_size)
	var pi := maxi(1, int(roundf(p)))
	var light_scale: float = knobs.light / 100.0
	var dots := {}
	if spec.uniform >= 0.0:
		var u := {"size": 1, "alpha": spec.uniform}
		dots = {Gridlite.Zone.RECT: u, Gridlite.Zone.BACK: u}
	else:
		for z in [[Gridlite.Zone.BACK, spec.back], [Gridlite.Zone.RECT, spec.rect]]:
			var light: float = Gridlite.dot_light(base, z[1], spec.elsh, font_size) * light_scale
			dots[z[0]] = Gridlite.whole_dot(light, pi, knobs.min_gap) if light > 0.0 else {"size": 0, "alpha": 0.0}
	var fill_spacing: bool = knobs.fill_spacing or spec.uniform >= 0.0
	var o := ci.get_global_transform_with_canvas().origin
	for zone in dots:
		var d: Dictionary = dots[zone]
		if d.size <= 0 or d.alpha <= 0.0:
			continue
		var n: int = d.size
		var col := Color(c, c.a * d.alpha)
		var half := Vector2(n, n) / 2.0
		var rect_size := Vector2(n, n)
		# the gap pixels of each dot's cell, at dot_gap_fill of the dot's light (only where
		# dot_min_gap keeps the dot smaller than its cell)
		var gap_rects: Array[Rect2] = []
		var gap_col := Color(c, c.a * d.alpha * knobs.gap_fill / 100.0)
		if knobs.min_gap > 0 and n < pi and gap_col.a > 0.0:
			gap_rects = _gap_rects(pi, n)
		for g in glyphs:
			if g.index < 0:
				continue
			var cells := Gridlite.cells(base, g.index)
			var cols: int = Gridlite.COLUMNS
			if fill_spacing and zone == Gridlite.Zone.RECT:
				cols = maxi(cols, int(roundf(g.advance / p)))
			for r in Gridlite.ROWS:
				var y: float = g.pen.y - (r + Gridlite.ROW_LOW) * p
				for k in cols:
					var z: int = cells[r * Gridlite.COLUMNS + k] if k < Gridlite.COLUMNS else Gridlite.Zone.RECT
					if z != zone:
						continue
					# a centre on a pixel corner takes the n × n pixels around it (n even), or
					# sits half a pixel right and below (n odd): whole canvas pixels either way
					var at: Vector2 = o + off + Vector2(g.pen.x + k * p, y) - half + Vector2(0.501, 0.501)
					var dot_at := at.floor() - o
					ci.draw_rect(Rect2(dot_at, rect_size), col)
					for r2 in gap_rects:
						ci.draw_rect(Rect2(dot_at + r2.position, r2.size), gap_col)


## The scaled-UI fallback of draw_glyph_zones(): every cell of `glyphs` filled flat, in
## all three zones, each at the light per area its dotted cell carries, so nothing is left
## to beat against the screen. RECT and BACK: the whole-pixel dot draw_glyph_zones() would
## draw (Gridlite.whole_dot) plus its gap fill, over the cell. Lit: the font's own lit dot
## at its wght (Gridlite.dot_light), over the cell. Runs of one zone in a row are one
## rect. Glyphs from a fallback font are drawn as glyphs.
static func draw_glyph_cells_solid(ci: CanvasItem, font: Font, font_size: int, glyphs: Array,
		c: Color, off: Vector2, knobs: Dictionary) -> void:
	var alpha := cell_alphas(font, font_size, knobs)
	if alpha.is_empty():
		return
	var spec := Gridlite.field_spec(font)
	var base: Font = (font as FontVariation).base_font
	var p := Gridlite.pitch_px(font_size)
	var fill_spacing: bool = knobs.fill_spacing or spec.uniform >= 0.0
	var ts := TextServerManager.get_primary_interface()
	for g in glyphs:
		if g.index < 0:
			if g.rid.is_valid() and g.glyph > 0:
				ts.font_draw_glyph(g.rid, ci.get_canvas_item(), font_size, g.pen + off, g.glyph, c)
			continue
		var cells := Gridlite.cells(base, g.index)
		var cols: int = Gridlite.COLUMNS
		if fill_spacing:
			# whole cells only: a cell past the advance would overlap the next glyph's
			cols = maxi(cols, int(floorf(g.advance / p + 0.01)))
		for r in Gridlite.ROWS:
			var top: float = g.pen.y - (r + Gridlite.ROW_LOW) * p - p / 2.0 + off.y
			var k := 0
			while k < cols:
				var z: int = cells[r * Gridlite.COLUMNS + k] if k < Gridlite.COLUMNS else Gridlite.Zone.RECT
				var run := 1
				while k + run < cols:
					var z2: int = cells[r * Gridlite.COLUMNS + k + run] if k + run < Gridlite.COLUMNS else Gridlite.Zone.RECT
					if z2 != z:
						break
					run += 1
				var a: float = alpha[z]
				if a > 0.0:
					var left: float = g.pen.x + k * p - p / 2.0 + off.x
					ci.draw_rect(Rect2(left, top, run * p, p), Color(c, c.a * a))
				k += run


## The light per area (an alpha, 0..1) of one cell of each zone at `font_size`, as the
## dotted field (and the font's lit dots) carry it: {Zone: alpha}. Empty for a font
## without a drawn field.
static func cell_alphas(font: Font, font_size: int, knobs: Dictionary) -> Dictionary:
	var spec := Gridlite.field_spec(font)
	if spec.is_empty():
		return {}
	var base: Font = (font as FontVariation).base_font
	var p := Gridlite.pitch_px(font_size)
	var pi := maxi(1, int(roundf(p)))
	var area := p * p
	var res := {}
	if spec.uniform >= 0.0:
		res[Gridlite.Zone.RECT] = spec.uniform / area
		res[Gridlite.Zone.BACK] = spec.uniform / area
	else:
		var light_scale: float = knobs.light / 100.0
		for z in [[Gridlite.Zone.BACK, spec.back], [Gridlite.Zone.RECT, spec.rect]]:
			var light: float = Gridlite.dot_light(base, z[1], spec.elsh, font_size) * light_scale
			var cell := 0.0
			if light > 0.0:
				var d := Gridlite.whole_dot(light, pi, knobs.min_gap)
				var n: int = d.size
				cell = d.alpha * n * n
				if knobs.min_gap > 0 and n < pi:
					cell += d.alpha * knobs.gap_fill / 100.0 * (pi * pi - n * n)
			res[z[0]] = minf(1.0, cell / area)
	var wght := Gridlite.axis(font as FontVariation, "wght", 400.0)
	res[Gridlite.Zone.LIT] = minf(1.0, Gridlite.dot_light(base, wght, spec.elsh, font_size) / area)
	return res


## The rest of a `pitch` x `pitch` cell around an `n` x `n` dot, as rects relative to the
## dot's top-left: the cell lies (pitch - n) / 2 px out from the dot on each side, rounded
## the way the dot's own centre rounds (an odd remainder puts the extra pixel left/above).
static func _gap_rects(pitch: int, n: int) -> Array[Rect2]:
	var lead := int(ceilf((pitch - n) / 2.0))
	var trail := pitch - n - lead
	var res: Array[Rect2] = []
	if lead > 0:
		res.append(Rect2(-lead, -lead, pitch, lead))  # above, full width
		res.append(Rect2(-lead, 0, lead, n))  # left
	if trail > 0:
		res.append(Rect2(-lead, n, pitch, trail))  # below, full width
		res.append(Rect2(n, 0, trail, n))  # right
	return res


## Where the font really draws a glyph whose pen position is `pen` (in `ci`'s px): on the
## whole canvas pixel at or left of and above it. With subpixel positioning off the glyph
## lands on whole pixels, and FreeType draws it on a whole-pixel baseline, so a fractional
## baseline (FontVariation.baseline_offset: -1.99 px at 26 px) rounds down.
static func _pen(pen: Vector2, ci: CanvasItem) -> Vector2:
	var o := ci.get_global_transform_with_canvas().origin
	return (o + pen).floor() - o


## Shapes `text` in `font`: one entry per glyph with its glyph index (-1: no dots), first
## character, advance and the shaper's offset (FontVariation.baseline_offset lands there).
static func _glyphs(font: Font, font_size: int, text: String) -> Array:
	var ts := TextServerManager.get_primary_interface()
	var line := TextLine.new()
	line.add_string(text, font, font_size)
	var own: RID = font.get_rids()[0]
	var res := []
	for g in ts.shaped_text_get_glyphs(line.get_rid()):
		# a glyph from a fallback font, or .notdef, keeps its advance but gets no dots
		var gridlite: bool = g.font_rid == own and g.index != 0
		res.append({"index": g.index if gridlite else -1, "start": g.start,
			"advance": g.advance * g.repeat, "offset": g.offset,
			"rid": RID() if gridlite else g.font_rid, "glyph": g.index})
	return res


## The state of the button `c` sits in: &"disabled", &"pressed" or &"" (none, or at rest).
static func _button_state(c: Control) -> StringName:
	var n := c.get_parent()
	while n != null:
		var b := n as BaseButton
		if b != null:
			if b.disabled:
				return &"disabled"
			return &"pressed" if b.is_pressed() else &""
		n = n.get_parent()
	return &""


static func _knobs(c: Control, own_type: bool, state := &"") -> Dictionary:
	var k := {"light": LIGHT_DEFAULT, "min_gap": MIN_GAP_DEFAULT, "fill_spacing": FILL_SPACING_DEFAULT != 0,
		"gap_fill": GAP_FILL_DEFAULT, "keep_pitch": SCALED_KEEP_PITCH_DEFAULT}
	if c == null:
		return k
	var t: StringName = &"" if own_type else &"Label"
	if c.has_theme_constant("dot_light", t):
		k.light = c.get_theme_constant("dot_light", t)
	var gap := &"dot_min_gap"
	if state != &"" and c.has_theme_constant(StringName("dot_min_gap_" + state), t):
		gap = StringName("dot_min_gap_" + state)
	if c.has_theme_constant(gap, t):
		k.min_gap = c.get_theme_constant(gap, t)
	var fill := &"dot_gap_fill"
	if state != &"" and c.has_theme_constant(StringName("dot_gap_fill_" + state), t):
		fill = StringName("dot_gap_fill_" + state)
	if c.has_theme_constant(fill, t):
		k.gap_fill = clampi(c.get_theme_constant(fill, t), 0, 100)
	if c.has_theme_constant("dot_fill_spacing", t):
		k.fill_spacing = c.get_theme_constant("dot_fill_spacing", t) != 0
	if c.has_theme_constant("dot_scaled_keep_pitch", t):
		k.keep_pitch = c.get_theme_constant("dot_scaled_keep_pitch", t)
	return k
