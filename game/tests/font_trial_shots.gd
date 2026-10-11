extends SceneTree
## Font trial for the neon chrome (chrome-dots, 2026-10-11): the start-up hub with the
## title and the button labels in a candidate dot font, lit dots only, over a dot field
## drawn by NeonDotField (ui/neon/neon_dot_field.gd). It changes only this run's
## in-memory theme; the game's defaults stay as they are. Runs on a private display:
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/font_trial_shots.gd -- --out DIR --variant C [--field-alpha T,C]
## Variants: A Gridlite today (its own unlit dots), B Gridlite lit only + field, C Doto
## (chrome 30 px, 1 px dots), C2 Doto (chrome 40 px, 2 px dots), D Punctum, E Matrix Mono
## Screen. The candidate fonts are not in git: see
## fonts/README.md. Writes DIR/hub-<variant>.png and DIR/hub-<variant>.json (each target
## label's rect, the font's pitch and the measured dot-centre phase in device px).

## Per font: file, units per em, dot pitch and first dot centre in font units (x from the
## pen position, y up from the baseline), hinting, and per role the size, axes, baseline
## offset and the lit dot's side in px at that size (for the report only).
const FONTS := {
	"B": {"name": "Gridlite PE (trial), lit dots only", "file": "res://fonts/GridlitePEVFTRIAL-All.ttf",
		"upm": 5200.0, "pitch": 400.0, "cx": 0.0, "cy": 0.0, "hinting": TextServer.HINTING_LIGHT,
		"title": {"size": 91, "axes": {"wght": 700.0, "BACK": 1.0, "RECT": 1.0, "ELSH": 3.5}, "baseline": -0.038},
		"chrome": {"size": 26, "axes": {"wght": 691.0, "BACK": 1.0, "RECT": 1.0, "ELSH": 2.65}, "baseline": -0.0765}},
	"C": {"name": "Doto", "file": "res://fonts/Doto[ROND,wght].ttf",
		"upm": 1000.0, "pitch": 100.0, "cx": 50.0, "cy": 50.0, "hinting": TextServer.HINTING_NONE,
		"title": {"size": 70, "axes": {"wght": 652.4, "ROND": 50.0}, "baseline": 0.0},
		"chrome": {"size": 30, "axes": {"wght": 144.5, "ROND": 0.0}, "baseline": 0.0}},
	"C2": {"name": "Doto, chrome at the next clean size", "file": "res://fonts/Doto[ROND,wght].ttf",
		"upm": 1000.0, "pitch": 100.0, "cx": 50.0, "cy": 50.0, "hinting": TextServer.HINTING_NONE,
		"title": {"size": 70, "axes": {"wght": 652.4, "ROND": 50.0}, "baseline": 0.0},
		"chrome": {"size": 40, "axes": {"wght": 366.7, "ROND": 0.0}, "baseline": 0.0}},
	"D": {"name": "Punctum", "file": "res://fonts/Punctum[ROND,wght].ttf",
		"upm": 1000.0, "pitch": 100.0, "cx": 0.0, "cy": 50.0, "hinting": TextServer.HINTING_NONE,
		"title": {"size": 80, "axes": {"wght": 609.3, "ROND": 50.0}, "baseline": 0.0},
		"chrome": {"size": 40, "axes": {"wght": 362.5, "ROND": 0.0}, "baseline": 0.0}},
	"E": {"name": "Matrix Mono Screen", "file": "res://fonts/MatrixMonoScreen-Regular.ttf",
		"upm": 1000.0, "pitch": 100.0, "cx": 0.0, "cy": 50.0, "hinting": TextServer.HINTING_NONE,
		"title": {"size": 70, "axes": {}, "baseline": 0.0},
		"chrome": {"size": 20, "axes": {}, "baseline": 0.0}},
}
## Gridlite today, for variant A's phase measurement.
const GRIDLITE := {"upm": 5200.0, "pitch": 400.0, "cx": 0.0, "cy": 0.0,
	"title": {"size": 91}, "chrome": {"size": 26}}

## The field: one whole pixel per dot, in the role's font colour (theme), at an alpha that
## gives each dot the light of Gridlite's own unlit (RECT) dot today. Overridable with
## --field-alpha TITLE,CHROME (measured from variant A against B).
var field_alpha := {"title": 0.86, "chrome": 0.51}
var field_dot := {"title": 1, "chrome": 1}
## Baseline correction in px, from checking the render (see _lattice).
var dy := 0.0

const DotField := preload("res://ui/neon/neon_dot_field.gd")

var out := ""
var variant := "A"
var main
var theme: Theme


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i] == "--out": out = args[i + 1]
		if args[i] == "--variant": variant = args[i + 1]
		if args[i] == "--field-alpha":
			var a := args[i + 1].split(",")
			field_alpha = {"title": float(a[0]), "chrome": float(a[1])}
		if args[i] == "--dy": dy = float(args[i + 1])
		if args[i] == "--field-dot":
			var d := args[i + 1].split(",")
			field_dot = {"title": int(d[0]), "chrome": int(d[1])}
	run()


func _font(spec: Dictionary, role: String) -> FontVariation:
	var ff := FontFile.new()
	ff.load_dynamic_font(ProjectSettings.globalize_path(spec.file))
	ff.antialiasing = TextServer.FONT_ANTIALIASING_GRAY
	ff.hinting = spec.hinting
	ff.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED
	ff.generate_mipmaps = false
	ff.multichannel_signed_distance_field = false
	var fv := FontVariation.new()
	fv.base_font = ff
	var ot := {}
	for tag in spec[role].axes:
		ot[TextServerManager.get_primary_interface().name_to_tag(tag)] = spec[role].axes[tag]
	fv.variation_opentype = ot
	fv.baseline_offset = spec[role].baseline
	return fv


func run() -> void:
	DirAccess.make_dir_recursive_absolute(out)
	theme = load("res://ui/neon/neon_theme.tres")
	var spec: Dictionary = FONTS.get(variant, {})
	if not spec.is_empty():
		theme.set_font("font", "TitleLabel", _font(spec, "title"))
		theme.set_font_size("font_size", "TitleLabel", spec.title.size)
		# the shadow stays one title dot deep
		var tp := int(round(spec.pitch * spec.title.size / spec.upm))
		theme.set_constant("shadow_offset_x", "TitleLabel", tp)
		theme.set_constant("shadow_offset_y", "TitleLabel", tp)
		theme.set_font("font", "ButtonLabelSecondary", _font(spec, "chrome"))
		theme.set_font_size("font_size", "ButtonLabelSecondary", spec.chrome.size)
	var saves := out.path_join("hub-saves-" + variant)
	preload("res://ui/saves.gd").remove_tree(saves)
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	root.add_child(main)
	DisplayServer.window_set_size(Vector2i(1920, 1080))
	for i in 20:
		await process_frame
	await RenderingServer.frame_post_draw
	var targets := _targets()
	var geo: Dictionary = spec if not spec.is_empty() else GRIDLITE
	var report := {"variant": variant, "font": spec.get("name", "Gridlite PE (trial), today"),
		"ui_scale": main.ui.scale.x, "labels": []}
	for t in targets:
		var l: Label = t.label
		var role: String = t.role
		var px: float = geo.pitch * geo[role].size / geo.upm
		var r := Rect2(l.get_global_transform_with_canvas().origin, l.size * main.ui.scale.x)
		var lat := _lattice(l, geo, role)
		report.labels.append({"text": l.text, "role": role, "rect": [r.position.x, r.position.y, r.size.x, r.size.y],
			"size": geo[role].size, "pitch_px": px, "baseline_y": lat.baseline, "first": [lat.first.x + r.position.x, lat.first.y + r.position.y],
			"phase_x": fposmod(lat.first.x + r.position.x, px), "phase_y": fposmod(lat.first.y + r.position.y, px),
			"button": _button_rect(l)})
		if not spec.is_empty():
			_add_field(l, role, lat, px)
	if not spec.is_empty():
		for i in 4:
			await process_frame
		await RenderingServer.frame_post_draw
	var img := root.get_texture().get_image()
	var path := out.path_join("hub-%s.png" % variant)
	img.save_png(path)
	var f := FileAccess.open(out.path_join("hub-%s.json" % variant), FileAccess.WRITE)
	f.store_string(JSON.stringify(report, "  "))
	print("SHOT ", path, " img=", img.get_size(), " ui.scale=", main.ui.scale.x)
	for e in report.labels:
		print("  ", e.role, " '", e.text, "' size=", e.size, " pitch=", e.pitch_px, " baseline=", e.baseline_y, " first=", e.first, " phase=(", e.phase_x, ", ", e.phase_y, ") rect=", e.rect)
	quit(0)


## The hub's title and every visible secondary button label.
func _targets() -> Array:
	var res := []
	for l in main.menu.find_children("*", "Label", true, false):
		if not l.is_visible_in_tree():
			continue
		if l.theme_type_variation == &"TitleLabel":
			res.append({"label": l, "role": "title"})
		elif l.theme_type_variation == &"ButtonLabelSecondary":
			res.append({"label": l, "role": "chrome"})
	return res


## The rect of the button holding label `l` (empty for the title), for the crops.
func _button_rect(l: Label) -> Array:
	var n: Node = l
	while n != null and not (n is BaseButton):
		n = n.get_parent()
	if n == null:
		return []
	var b: Control = n
	var o := b.get_global_transform_with_canvas().origin
	return [o.x, o.y, b.size.x, b.size.y]


## Where the font's dot lattice lies in label `l`: the baseline (global y) as Label centres
## its one line, the first dot centre (label-local, from the pen start and the line box's
## top) and the field's extent (the text's advance by the line box). --dy shifts the
## baseline for a check against the render.
func _lattice(l: Label, spec: Dictionary, role: String) -> Dictionary:
	var font: Font = l.get_theme_font("font")
	var size: int = l.get_theme_font_size("font_size")
	var asc := font.get_ascent(size)
	var desc := font.get_descent(size)
	var base := floorf((l.size.y - (asc + desc)) / 2.0 + asc) + dy
	# FontVariation.baseline_offset moves the glyphs by a fraction of the font's height
	if font is FontVariation:
		base += (font as FontVariation).baseline_offset * (asc + desc)
	var adv := font.get_string_size(l.text.to_upper() if l.uppercase else l.text, HORIZONTAL_ALIGNMENT_LEFT, -1, size).x
	var s: float = size / spec.upm
	var px: float = spec.pitch * s
	# rows: the lowest centre at or above the line box's bottom, up past its top
	var cy: float = base - spec.cy * s
	var k := floorf((base + desc - cy) / px)
	var fy := cy + k * px
	while fy - px >= base - asc - 0.001:
		fy -= px
	return {"baseline": l.get_global_transform_with_canvas().origin.y + base,
		"first": Vector2(spec.cx * s, fy), "extent": Vector2(adv, base + desc + px / 2.0)}


## The field behind one label: dots on the font's lattice across the text's advance and
## its line box.
func _add_field(l: Label, role: String, lat: Dictionary, px: float) -> void:
	var field := DotField.new()
	field.show_behind_parent = true
	field.mouse_filter = Control.MOUSE_FILTER_IGNORE
	field.position = Vector2.ZERO
	field.size = lat.extent
	field.first = lat.first
	field.pitch = int(round(px))
	field.dot_size = field_dot[role]
	var c: Color = l.get_theme_color("font_color")
	c.a = field_alpha[role]
	field.color = c
	l.add_child(field)
