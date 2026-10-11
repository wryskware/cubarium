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
## - dot_fill_spacing: 1 also fills the letter-spacing columns (spacing_glyph) with RECT
##   dots; 0 leaves them empty, as the font does.
##
## Uniform mode (follow_label off; the chrome-dots trial, tests/font_trial_shots.gd): a
## `dot_size` square of `color` at `first` + k × `pitch` across the control's rect.

enum Layer { TEXT, SHADOW }

const LIGHT_DEFAULT := 100
const MIN_GAP_DEFAULT := 0
const FILL_SPACING_DEFAULT := 0

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
	if not l.is_visible_in_tree() or l.text == "":
		return
	var font := l.get_theme_font("font")
	var spec := Gridlite.field_spec(font)
	if spec.is_empty():
		return
	var c := l.get_theme_color("font_color")
	var off := Vector2.ZERO
	if layer == Layer.SHADOW:
		c = l.get_theme_color("font_shadow_color")
		off = Vector2(l.get_theme_constant("shadow_offset_x"), l.get_theme_constant("shadow_offset_y"))
	if c.a <= 0.0:
		return
	var font_size := l.get_theme_font_size("font_size")
	var text := l.text.to_upper() if l.uppercase else l.text
	var glyphs := _glyphs(font, font_size, text)
	# each glyph's pen position from the label's own layout (alignment, wrapping)
	var asc := font.get_ascent(font_size)
	for g in glyphs:
		var b := l.get_character_bounds(g.start)
		if b.size == Vector2.ZERO:
			g.index = -1  # not shown (clipped, trimmed or past visible_characters)
		g.pen = _pen(Vector2(b.position.x, b.position.y + asc) + g.offset, l)
	draw_glyph_zones(self, font, font_size, glyphs, c, off, _knobs(l, true))


## The drawn Gridlite field for text a control draws itself with draw_string(): call it
## from that control's _draw() with draw_string's own arguments. Draws nothing for a font
## without a drawn field.
static func draw_string_field(ci: CanvasItem, font: Font, pos: Vector2, text: String,
		alignment := HORIZONTAL_ALIGNMENT_LEFT, width := -1.0, font_size := 16,
		modulate := Color.WHITE) -> void:
	if Gridlite.field_spec(font).is_empty() or text == "":
		return
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
		g.pen = _pen(Vector2(x, pos.y) + g.offset, ci)
		x += g.advance
	var knobs := _knobs(ci as Control, false) if ci is Control else _knobs(null, false)
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
					ci.draw_rect(Rect2(at.floor() - o, rect_size), col)


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
			"advance": g.advance * g.repeat, "offset": g.offset})
	return res


static func _knobs(c: Control, own_type: bool) -> Dictionary:
	var k := {"light": LIGHT_DEFAULT, "min_gap": MIN_GAP_DEFAULT, "fill_spacing": FILL_SPACING_DEFAULT != 0}
	if c == null:
		return k
	var t: StringName = &"" if own_type else &"Label"
	if c.has_theme_constant("dot_light", t):
		k.light = c.get_theme_constant("dot_light", t)
	if c.has_theme_constant("dot_min_gap", t):
		k.min_gap = c.get_theme_constant("dot_min_gap", t)
	if c.has_theme_constant("dot_fill_spacing", t):
		k.fill_spacing = c.get_theme_constant("dot_fill_spacing", t) != 0
	return k
