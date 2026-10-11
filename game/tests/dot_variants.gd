extends RefCounted
## Switches the neon theme's Gridlite text between the chrome-dots ways of drawing its
## unlit dots, for side-by-side pictures (2026-10-11). It changes the loaded theme and font
## resources in memory for one run. The shot scripts take it as `--dots VARIANT`:
## - new: the game's default. Gridlite's own BACK/RECT off; NeonDotField draws both zones
##   with the role's values (fonts' dot_back/dot_rect metadata). Same as no --dots.
## - A: Gridlite as before the drawn field: the font draws its own unlit dots (BACK/RECT
##   set back from the metadata) and nothing is drawn for it.
## - B: the trial's variant B rule: lit dots only over one flat 1 px field across the whole
##   advance (letter spacing too), alpha 0.86 for the title font, 0.51 for the chrome fonts.
## - gap1: new, with dots kept a pixel apart (dot_min_gap 1): plates stay dotted at a 2 px
##   pitch, but too dim to carry the font's light.
## - fill: new, with the letter-spacing columns filled with RECT dots (dot_fill_spacing 1).
## - bright: new, with plain chrome's drawn dots (chrome fonts with BACK and RECT under 200:
##   not the second button's plate, the negative or the pressed plate) at dot_light 270 %:
##   about the light trial B's flat 0.51 field puts on plain chrome labels, keeping the
##   BACK:RECT ratio. (B's 0.51 matches today's light on the second button's plate.)

const TITLE_FONT := "res://ui/neon/fonts/title.tres"
const B_ALPHA := {"title": 0.86, "chrome": 0.51}
const BRIGHT_CHROME := 270
const THEME := "res://ui/neon/neon_theme.tres"


## The changed theme, held so the resource cache keeps it (and its fonts) for the run.
static var _held: Theme


## Applies `variant` to the neon theme (call before the scene is made).
static func apply_neon(variant: String) -> void:
	_held = load(THEME)
	apply(_held, variant)


static func apply(theme: Theme, variant: String) -> void:
	match variant:
		"", "new":
			return
		"gap1":
			theme.set_constant("dot_min_gap", "Label", 1)
			return
		"fill":
			theme.set_constant("dot_fill_spacing", "Label", 1)
			return
	var done := {}
	for type in theme.get_font_type_list():
		if not theme.has_font("font", type):
			continue
		var f := theme.get_font("font", type) as FontVariation
		if Gridlite.field_spec(f).is_empty():
			continue
		var role := "title" if f.resource_path == TITLE_FONT else "chrome"
		match variant:
			"bright":
				var spec := Gridlite.field_spec(f)
				if role == "chrome" and spec.back < 200.0 and spec.rect < 200.0:
					theme.set_constant("dot_light", type, BRIGHT_CHROME)
			"A", "B":
				if not done.has(f):
					done[f] = true
					_to_variant(f, variant, role)
			_:
				push_error("dot_variants: unknown variant %s" % variant)
				return


## Turns the game's font `f` into the variant's, in place: the loaded resource is shared,
## so controls that preload it (NeonDotSlider, NeonDotProgress) follow too.
static func _to_variant(v: FontVariation, variant: String, role: String) -> void:
	var spec := Gridlite.field_spec(v)
	for m in [&"dot_back", &"dot_rect", &"dot_uniform"]:
		if v.has_meta(m):
			v.remove_meta(m)
	if variant == "A":
		var ot := v.variation_opentype.duplicate()
		ot[Gridlite.tag("BACK")] = spec.back
		ot[Gridlite.tag("RECT")] = spec.rect
		v.variation_opentype = ot
	else:
		v.set_meta(&"dot_uniform", B_ALPHA[role])
