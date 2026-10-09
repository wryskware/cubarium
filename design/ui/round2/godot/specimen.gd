extends Control
## Cubarium UI round 2: the Lanternjaw field-guide page as Godot draws it, to judge the
## three type roles (title, text, chrome) in the engine. Round 2's HTML page
## (../index.html) is the reference layout; the UI draws at native pixels like the game.
##
##   godot --path design/ui/round2/godot [--resolution 2560x1440] [-- OPTIONS]
##
## Options:
##   --gridlite PATH   Gridlite PE variable font (default ~/Downloads/GridlitePEVFTRIAL-All.ttf,
##                     a trial licensed for testing only: never commit it)
##   --text pixelify|pixeloid|doto|gridlite   text font (default pixelify)
##   --text-px N       off-grid text: the field note's size in px (default 23)
##   --text-step N     grid text: steps up or down from the automatic multiple
##   --text-wght N     Pixelify Sans weight, 400 to 700 (default 400)
##   --text-aa auto|gray|none        --text-hint auto|none|light|normal
##   --bright          round 2's brighter chrome colours
##   --info            show the settings line
##   --shot FILE       write one frame to FILE and quit (run it under scripts/hidden.sh)
## Keys: T text font · [ ] text size · W Pixelify weight · A text antialiasing ·
##       H text hinting · C chrome colour · I settings line · F11 fullscreen · Esc quit

const W := 1920
const H := 1080
const INK := Color("#F7F0FF")
const INK2 := Color("#DCD1F7")
const ACCENT := Color("#42C5F8")
const WARM := Color("#FDA243")
const HOT := Color("#F25BDB")
const LINE := Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.32)
const PANEL_RING := Color("#C23FD3")
const BANDS := [[0.18, Color("#2A1470")], [0.36, Color("#24115F")], [0.56, Color("#1D0D50")],
		[0.76, Color("#160A41")], [1.0, Color("#100632")]]
# Sample numbers avoid the digit 2: the Gridlite trial replaces 2, Q, X, q and x with a
# placeholder box (the bought font has them).
const ANIMALS := [["Frondgrazer", "#FB8162", 41], ["Littershredder", "#FED24E", 118],
		["Bellwing", "#C19BFC", 33], ["Lanternjaw", "#9168A1", 7], ["Veilhound", "#6C55D5", 4],
		["Loftstrider", "#22B4D2", 3], ["Capgnawer", "#A76982", 31], ["Ripple-snail", "#5081F2", 56],
		["Seedporter", "#889335", 19]]
const PREY := [["Littershredders", "#FED24E"], ["Frondgrazers", "#FB8162"], ["Capgnawers", "#A76982"],
		["Ripple-snails", "#5081F2"], ["Resting bellwings", "#C19BFC"],
		["Grounded seedporters", "#889335"], ["Carrion", "#A595D8"]]
const SERIES := [4, 4, 5, 5, 5, 6, 6, 6, 5, 5, 6, 7, 7, 7, 8, 8, 7, 7, 7, 6, 6, 7, 7, 8, 8, 8, 7, 7,
		7, 7, 8, 7, 7, 7, 7, 7]
const FIELD := "A low violet scorpid that hunts by waiting. Its forked tail arches over its back and holds two cyan lures before its face, dead still, until prey comes within reach of its mantis striking arms. Keeping the lures costs it reserve. At the edge of another hunter's kill, it takes what it can."

# Wrysk's picks, 2026-10-09. Gridlite's grid: 13 px a dot at 1x, capitals 7 dots.
const TITLE_AXES := {"wght": 700, "BACK": 90, "RECT": 120, "ELSH": 3.5}
const CHROME_AXES := {"wght": 691, "BACK": 135, "RECT": 71, "ELSH": 2.65}
const TITLE_PX := 91 # 13 x 7
const BIG_PX := 52 # 13 x 4
const CHROME_PX := 26 # 13 x 2
const CHROME_SPACING := 2 # one grid step of letter spacing for upper case

# Text candidates: grid = px a font pixel at 1x (0 = not on a strict grid), cap = capital
# height in font pixels, aa = default antialiasing (pixel fonts draw unsmoothed).
const TEXTS := {
	"pixelify": {"name": "Pixelify Sans", "file": "pixelify-sans/PixelifySans[wght].ttf", "grid": 0, "cap": 0, "aa": "gray"},
	"pixeloid": {"name": "Pixeloid Sans", "file": "pixeloid-sans/PixeloidSans.ttf", "grid": 9, "cap": 7, "aa": "none"},
	"doto": {"name": "Doto", "file": "doto/Doto[ROND,wght].ttf", "grid": 10, "cap": 7, "aa": "none", "axes": {"wght": 900, "ROND": 0}},
	"gridlite": {"name": "Gridlite PE, plain square", "file": "", "grid": 13, "cap": 7, "aa": "gray", "axes": {"wght": 700, "BACK": 1, "RECT": 1, "ELSH": 3}},
}
const TEXT_ORDER := ["pixelify", "pixeloid", "doto", "gridlite"]
const AA := {"gray": TextServer.FONT_ANTIALIASING_GRAY, "none": TextServer.FONT_ANTIALIASING_NONE}
const HINT := {"none": TextServer.HINTING_NONE, "light": TextServer.HINTING_LIGHT, "normal": TextServer.HINTING_NORMAL}

const SCRIM := """
shader_type canvas_item;
void fragment() {
	vec2 cell = floor(FRAGCOORD.xy / 3.0);
	COLOR = vec4(7.0 / 255.0, 4.0 / 255.0, 26.0 / 255.0, mix(0.62, 0.88, mod(cell.x + cell.y, 2.0)));
}
"""

var text_key := "pixelify"
var text_px := 23 # Wrysk 2026-10-09: "i kinda still want pixelify at 23px"
var text_step := 0
var text_wght := 400
var text_aa := "auto"
var text_hint := "auto"
var bright := false
var info := false
var shot := ""
var gridlite_path := ""
var fonts_dir := ""
var assets_dir := ""

var world_tex: Texture2D
var art_tex: Texture2D
var page: Control
var info_label: Label

# role fonts, rebuilt on every change
var f_title: Font
var f_chrome: Font
var f_key: Font
var f_text: Font
var t1 := 21
var t2 := 19
var t3 := 16
var lab := Color("#A595D8")
var tab := Color("#A595D8")
var kick := HOT
var tabs_bg := Color(11 / 255.0, 5 / 255.0, 37 / 255.0, 0.70)


class Box extends MarginContainer:
	## A margin container that paints round 2's panel language behind its children:
	## flat or banded fill, the 3 px notched ring, a dashed top rule, a soft glow.
	var bg := Color(0, 0, 0, 0)
	var bands: Array = []
	var ring := Color(0, 0, 0, 0)
	var dash_top := Color(0, 0, 0, 0)
	var edge_right := Color(0, 0, 0, 0)
	var glow := Color(0, 0, 0, 0)
	var glow_px := 26.0
	var inset_hi := Color(0, 0, 0, 0)
	var inset_lo := Color(0, 0, 0, 0)
	var outline := Color(0, 0, 0, 0)

	func _init() -> void:
		resized.connect(queue_redraw)

	func pad(t: int, r: int, b: int, l: int) -> Box:
		add_theme_constant_override("margin_top", t)
		add_theme_constant_override("margin_right", r)
		add_theme_constant_override("margin_bottom", b)
		add_theme_constant_override("margin_left", l)
		return self

	func _draw() -> void:
		var w := size.x
		var h := size.y
		if glow.a > 0.0:
			var n := 12
			for i in n:
				var g := glow_px * (i + 1) / n
				draw_rect(Rect2(-g, -g, w + 2.0 * g, h + 2.0 * g),
						Color(glow.r, glow.g, glow.b, glow.a * 0.35 / n))
		if not bands.is_empty():
			var y0 := 0.0
			for b in bands:
				var y1: float = roundf(h * float(b[0]))
				draw_rect(Rect2(0, y0, w, y1 - y0), b[1])
				y0 = y1
		elif bg.a > 0.0:
			draw_rect(Rect2(0, 0, w, h), bg)
		if inset_hi.a > 0.0:
			draw_rect(Rect2(0, 0, w, 3), inset_hi)
			draw_rect(Rect2(0, h - 3, w, 3), inset_lo)
		if ring.a > 0.0:
			draw_rect(Rect2(0, -3, w, 3), ring)
			draw_rect(Rect2(0, h, w, 3), ring)
			draw_rect(Rect2(-3, 0, 3, h), ring)
			draw_rect(Rect2(w, 0, 3, h), ring)
		if dash_top.a > 0.0:
			var x := 0.0
			while x < w:
				draw_rect(Rect2(x, 0, minf(3.0, w - x), 3), dash_top)
				x += 6.0
		if edge_right.a > 0.0:
			draw_rect(Rect2(w - 1, 0, 1, h), edge_right)
		if outline.a > 0.0:
			draw_rect(Rect2(-10, -10, w + 20, 3), outline)
			draw_rect(Rect2(-10, h + 7, w + 20, 3), outline)
			draw_rect(Rect2(-10, -7, 3, h + 14), outline)
			draw_rect(Rect2(w + 7, -7, 3, h + 14), outline)


class Paint extends Control:
	## A control drawn by a callable(canvas_item, size).
	var painter: Callable

	func _init(p: Callable, min_size := Vector2.ZERO) -> void:
		painter = p
		custom_minimum_size = min_size
		mouse_filter = Control.MOUSE_FILTER_IGNORE
		resized.connect(queue_redraw)

	func _draw() -> void:
		painter.call(self, size)


func _ready() -> void:
	var base := ProjectSettings.globalize_path("res://")
	fonts_dir = base.path_join("../fonts").simplify_path()
	assets_dir = base.path_join("../../round1/assets").simplify_path()
	gridlite_path = OS.get_environment("HOME").path_join("Downloads/GridlitePEVFTRIAL-All.ttf")
	parse_args()
	world_tex = load_texture(assets_dir.path_join("world.png"))
	art_tex = load_texture(assets_dir.path_join("lanternjaw-px.png"))
	get_viewport().size_changed.connect(place_page)
	rebuild()
	if shot != "":
		for i in 4:
			await get_tree().process_frame
		await RenderingServer.frame_post_draw
		var err := get_viewport().get_texture().get_image().save_png(shot)
		print("specimen: wrote %s (%s)" % [shot, error_string(err)])
		get_tree().quit(0 if err == OK else 1)


func parse_args() -> void:
	var args := OS.get_cmdline_user_args()
	var i := 0
	while i < args.size():
		var a := args[i]
		var v := args[i + 1] if i + 1 < args.size() else ""
		match a:
			"--gridlite":
				gridlite_path = v
				i += 1
			"--text":
				text_key = v if TEXTS.has(v) else text_key
				i += 1
			"--text-px":
				text_px = int(v)
				i += 1
			"--text-step":
				text_step = int(v)
				i += 1
			"--text-wght":
				text_wght = int(v)
				i += 1
			"--text-aa":
				text_aa = v
				i += 1
			"--text-hint":
				text_hint = v
				i += 1
			"--shot":
				shot = v
				i += 1
			"--bright":
				bright = true
			"--info":
				info = true
		i += 1


func load_texture(path: String) -> Texture2D:
	var img := Image.load_from_file(path)
	if img == null:
		push_error("specimen: can't load %s" % path)
		return null
	return ImageTexture.create_from_image(img)


func font_file(path: String, aa: int, hint: int, on_grid: bool) -> Font:
	if not FileAccess.file_exists(path):
		push_error("specimen: no font at %s" % path)
		return ThemeDB.fallback_font
	var f := FontFile.new()
	if f.load_dynamic_font(path) != OK:
		push_error("specimen: can't read font %s" % path)
		return ThemeDB.fallback_font
	f.antialiasing = aa
	f.hinting = hint
	f.subpixel_positioning = TextServer.SUBPIXEL_POSITIONING_DISABLED if on_grid else TextServer.SUBPIXEL_POSITIONING_AUTO
	f.generate_mipmaps = false
	return f


func variation(base: Font, axes: Dictionary, spacing := 0) -> Font:
	var ts := TextServerManager.get_primary_interface()
	var coords := {}
	for k in axes:
		coords[ts.name_to_tag(k)] = axes[k]
	var v := FontVariation.new()
	v.base_font = base
	v.variation_opentype = coords
	v.spacing_glyph = spacing
	return v


func setup_fonts() -> void:
	var gl := gridlite_path
	f_title = variation(font_file(gl, AA["gray"], HINT["none"], true), TITLE_AXES)
	var chrome_base := font_file(gl, AA["gray"], HINT["none"], true)
	f_chrome = variation(chrome_base, CHROME_AXES, CHROME_SPACING)
	f_key = variation(chrome_base, CHROME_AXES)
	var t: Dictionary = TEXTS[text_key]
	var grid: int = t["grid"]
	var aa_name: String = t["aa"] if text_aa == "auto" else text_aa
	var hint_name: String = ("light" if grid == 0 else "none") if text_hint == "auto" else text_hint
	var path: String = gl if t["file"] == "" else fonts_dir.path_join(t["file"])
	var base := font_file(path, AA.get(aa_name, AA["gray"]), HINT.get(hint_name, HINT["light"]), grid > 0)
	var axes: Dictionary = t.get("axes", {})
	if text_key == "pixelify":
		axes = {"wght": text_wght}
	f_text = variation(base, axes) if not axes.is_empty() else base
	if grid > 0:
		var cap: int = t["cap"]
		var p := grid * maxi(1, roundi(14.0 / cap) + text_step)
		t1 = p
		t2 = p
		t3 = p
	else:
		# off-grid text steps by whole pixels; lists and chips keep round 1's ratios
		t1 = text_px
		t2 = roundi(text_px * 19.0 / 21.0)
		t3 = roundi(text_px * 16.0 / 21.0)
	lab = Color("#D9CEF7") if bright else Color("#A595D8")
	tab = Color("#E6DEFF") if bright else Color("#A595D8")
	kick = Color("#FF9BEA") if bright else HOT
	tabs_bg = Color(11 / 255.0, 5 / 255.0, 37 / 255.0, 0.88 if bright else 0.70)


func settings_line() -> String:
	var t: Dictionary = TEXTS[text_key]
	var size_note := "%d px" % t1 if int(t["grid"]) > 0 else "%d / %d / %d px" % [t1, t2, t3]
	var w := " wght %d" % text_wght if text_key == "pixelify" else ""
	return "Text: %s %s%s · aa %s · hinting %s · chrome %s   [T font · [ ] size · W weight · A aa · H hinting · C colour · I hide]" % [
			t["name"], size_note, w, text_aa, text_hint, "brighter" if bright else "round 1"]


# ---------------------------------------------------------------- building blocks

func label(text: String, font: Font, px: int, color: Color, upper := false) -> Label:
	var l := Label.new()
	l.text = text
	l.uppercase = upper
	l.add_theme_font_override("font", font)
	l.add_theme_font_size_override("font_size", px)
	l.add_theme_color_override("font_color", color)
	l.add_theme_constant_override("line_spacing", 0)
	l.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	l.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return l


func chrome(text: String, color: Color) -> Label:
	return label(text, f_chrome, CHROME_PX, color, true)


func body(text: String, px: int, color: Color) -> Label:
	return label(text, f_text, px, color)


func hbox(sep: int, kids: Array = []) -> HBoxContainer:
	var b := HBoxContainer.new()
	b.add_theme_constant_override("separation", sep)
	for k in kids:
		b.add_child(k)
	return b


func vbox(sep: int, kids: Array = []) -> VBoxContainer:
	var b := VBoxContainer.new()
	b.add_theme_constant_override("separation", sep)
	for k in kids:
		b.add_child(k)
	return b


func spacer(min_px := 0, vertical := false) -> Control:
	var c := Control.new()
	c.mouse_filter = Control.MOUSE_FILTER_IGNORE
	if vertical:
		c.custom_minimum_size.y = min_px
	else:
		c.custom_minimum_size.x = min_px
	return c


func grow(c: Control, horizontal := true) -> Control:
	if horizontal:
		c.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	else:
		c.size_flags_vertical = Control.SIZE_EXPAND_FILL
	return c


func box(kid: Control = null) -> Box:
	var b := Box.new()
	b.pad(0, 0, 0, 0)
	if kid != null:
		b.add_child(kid)
	return b


func dot(color: Color, px: int) -> Control:
	var d := ColorRect.new()
	d.color = color
	d.custom_minimum_size = Vector2(px, px)
	d.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	d.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	d.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return d


func keycap(text: String, color: Color) -> Box:
	# the trial has no X: that one keycap borrows the text font
	var l := label(text, f_text if text == "X" else f_key, CHROME_PX, color)
	l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	l.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	var b := box(l).pad(0, 6, 0, 6)
	b.ring = color
	b.custom_minimum_size = Vector2(CHROME_PX + 12, CHROME_PX + 12)
	b.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	return b


func pixels(pattern: Array, scale: int, color: Color) -> Control:
	var w := 0
	var h := 0
	for p in pattern:
		w = maxi(w, p[0] + 1)
		h = maxi(h, p[1] + 1)
	var paint := func(ci: CanvasItem, s: Vector2) -> void:
		var oy := floorf((s.y - h * scale) / 2.0)
		for p in pattern:
			ci.draw_rect(Rect2(p[0] * scale, oy + p[1] * scale, scale, scale), color)
	var c := Paint.new(paint, Vector2(w * scale, h * scale))
	c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	return c


# ---------------------------------------------------------------- the page

func rebuild() -> void:
	for c in get_children():
		c.queue_free()
	setup_fonts()
	var world := TextureRect.new()
	world.texture = world_tex
	world.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	world.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
	world.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	world.set_anchors_preset(Control.PRESET_FULL_RECT)
	add_child(world)
	var scrim := ColorRect.new()
	var mat := ShaderMaterial.new()
	mat.shader = Shader.new()
	mat.shader.code = SCRIM
	scrim.material = mat
	scrim.set_anchors_preset(Control.PRESET_FULL_RECT)
	add_child(scrim)
	page = Control.new()
	page.size = Vector2(W, H)
	page.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(page)
	build_top()
	build_body()
	build_hints()
	info_label = Label.new()
	info_label.text = settings_line()
	info_label.add_theme_font_size_override("font_size", 14)
	info_label.add_theme_color_override("font_color", Color.WHITE)
	info_label.add_theme_color_override("font_outline_color", Color.BLACK)
	info_label.add_theme_constant_override("outline_size", 5)
	info_label.position = Vector2(10, 6)
	info_label.visible = info
	add_child(info_label)
	place_page()


func place_page() -> void:
	if page == null:
		return
	var vs := get_viewport_rect().size
	page.position = Vector2(floorf((vs.x - W) / 2.0), floorf((vs.y - H) / 2.0))


func build_top() -> void:
	var tabs_row := hbox(6)
	for n in ["Field guide", "Populations", "World", "Settings"]:
		var on: bool = n == "Field guide"
		var t := box(chrome(n, Color.WHITE if on else tab)).pad(0, 18, 0, 18)
		t.custom_minimum_size.y = roundi(CHROME_PX * 1.15 + 22)
		if on:
			t.bg = Color("#8A2BA6")
			t.ring = HOT
		tabs_row.add_child(t)
	var tabs := box(tabs_row).pad(6, 6, 6, 6)
	tabs.bg = tabs_bg
	tabs.ring = LINE
	tabs.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	var paused := box(chrome("Paused", WARM)).pad(6, 10, 6, 10)
	paused.bg = Color(253 / 255.0, 162 / 255.0, 67 / 255.0, 0.12)
	paused.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	var clock := hbox(12, [chrome("Day", lab), chrome("3", INK2), chrome("14:30", INK2), paused])
	clock.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	var top := hbox(0, [tabs, grow(spacer()), clock])
	top.position = Vector2(96, 40)
	top.size = Vector2(1728, 64)
	page.add_child(top)


func build_body() -> void:
	var clip := Control.new()
	clip.clip_contents = true
	clip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var panel := box(clip)
	panel.bands = BANDS
	panel.ring = PANEL_RING
	panel.glow = Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.30)
	panel.glow_px = 48.0
	panel.position = Vector2(96, 128)
	panel.size = Vector2(1728, 848)
	page.add_child(panel)
	var row := hbox(0, [build_index(), grow(build_entry())])
	row.set_anchors_preset(Control.PRESET_FULL_RECT)
	clip.add_child(row)


func build_index() -> Control:
	var magnifier := pixels([[2, 0], [3, 0], [4, 0], [1, 1], [5, 1], [0, 2], [6, 2], [0, 3], [6, 3], [0, 4],
			[6, 4], [1, 5], [5, 5], [2, 6], [3, 6], [4, 6], [6, 6], [7, 7], [8, 8]], 2, lab)
	var search := box(hbox(12, [magnifier, grow(body("Search species", t3, lab))])).pad(0, 14, 0, 14)
	search.bg = Color(11 / 255.0, 5 / 255.0, 37 / 255.0, 0.5)
	search.ring = LINE
	search.custom_minimum_size.y = 46
	var col := vbox(0)
	col.add_child(box(search).pad(0, 28, 18, 28))
	col.add_child(box(hbox(0, [chrome("Animals", lab), grow(spacer()), chrome("9", INK2)])).pad(16, 28, 10, 28))
	var rows := vbox(0)
	for a in ANIMALS:
		var sel: bool = a[0] == "Lanternjaw"
		var r := box(hbox(14, [dot(Color(a[1]), 10), body(a[0], t2, Color.WHITE if sel else INK2),
				grow(spacer()), chrome(str(a[2]), INK2)])).pad(0, 16, 0, 16)
		r.custom_minimum_size.y = 46
		if sel:
			r.bg = Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.35)
			r.ring = Color(242 / 255.0, 91 / 255.0, 219 / 255.0, 0.6)
		rows.add_child(r)
	col.add_child(box(rows).pad(0, 14, 0, 14))
	col.add_child(grow(spacer(), false))
	var chev := pixels([[0, 0], [1, 1], [2, 2], [1, 3], [0, 4]], 2, lab)
	var closed := box(hbox(0, [hbox(10, [chev, chrome("Plants", lab)]), grow(spacer()), chrome("10", INK2)])).pad(20, 28, 10, 28)
	closed.dash_top = LINE
	col.add_child(closed)
	var index := box(col).pad(28, 0, 28, 0)
	index.custom_minimum_size.x = 400
	index.edge_right = LINE
	return index


func build_entry() -> Control:
	var entry := hbox(56, [build_platecol(), grow(build_textcol())])
	return box(entry).pad(48, 56, 48, 56)


func build_platecol() -> Control:
	var D := "·"
	var plate := box()
	plate.ring = LINE
	plate.custom_minimum_size = Vector2(540, 500)
	plate.clip_contents = true
	plate.add_child(Paint.new(paint_plate))
	var art := TextureRect.new()
	art.texture = art_tex
	art.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	art.stretch_mode = TextureRect.STRETCH_SCALE
	art.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	art.custom_minimum_size = Vector2(528, 489)
	var center := CenterContainer.new()
	center.add_child(art)
	plate.add_child(center)
	var cap := chrome("Plate 04 %s Adult" % D, INK2)
	cap.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	cap.size_flags_vertical = Control.SIZE_SHRINK_BEGIN
	plate.add_child(box(cap).pad(10, 0, 0, 12))

	var big := label("7", f_title, BIG_PX, ACCENT)
	var stages := body("4 adults %s 3 juveniles" % D, t3, INK2)
	var count := vbox(10, [big, stages])
	count.size_flags_vertical = Control.SIZE_SHRINK_END
	var bars := grow(Paint.new(paint_bars, Vector2(0, 64)))
	var axis := hbox(0, [chrome("−6 h", lab), grow(spacer()), chrome("Now", lab)])
	var chart := grow(vbox(8, [bars, axis]))
	chart.size_flags_vertical = Control.SIZE_SHRINK_END
	var live_col := vbox(0, [chrome("In this world", lab), grow(spacer(), false), hbox(28, [count, chart])])
	var live := box(live_col).pad(20, 24, 20, 24)
	live.ring = LINE
	live.size_flags_vertical = Control.SIZE_EXPAND_FILL
	var col := vbox(24, [plate, live])
	col.custom_minimum_size.x = 540
	return col


func build_textcol() -> Control:
	var D := "·"
	var col := vbox(0)
	col.add_child(spacer(6, true))
	col.add_child(chrome("Animal %s Predator" % D, kick))
	col.add_child(spacer(14, true))
	var title := label("Lanternjaw", f_title, TITLE_PX, INK)
	# the offset shadow is one title pixel (13 px grid x 7)
	title.add_theme_color_override("font_shadow_color", Color("#5A1D9D"))
	title.add_theme_constant_override("shadow_offset_x", 7)
	title.add_theme_constant_override("shadow_offset_y", 7)
	title.add_theme_constant_override("shadow_outline_size", 0)
	col.add_child(title)
	col.add_child(spacer(6, true))
	col.add_child(body("[binomial]", t3, lab))
	col.add_child(spacer(28, true))
	var dashes := func(ci: CanvasItem, s: Vector2) -> void:
		var x := 0.0
		while x < s.x:
			ci.draw_rect(Rect2(x, 0, 3, 3), HOT)
			x += 6.0
	col.add_child(Paint.new(dashes, Vector2(0, 3)))
	col.add_child(spacer(28, true))
	var field := body(FIELD, t1, INK2)
	field.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	field.size_flags_horizontal = Control.SIZE_FILL
	field.add_theme_constant_override("line_spacing", roundi(1.5 * t1 - f_text.get_height(t1)))
	col.add_child(field)
	col.add_child(spacer(28, true))
	var facts := GridContainer.new()
	facts.columns = 2
	facts.add_theme_constant_override("h_separation", 0)
	facts.add_theme_constant_override("v_separation", 0)
	var chips := HFlowContainer.new()
	chips.add_theme_constant_override("h_separation", 10)
	chips.add_theme_constant_override("v_separation", 10)
	for p in PREY:
		var c := box(hbox(8, [dot(Color(p[1]), 8), body(p[0], t3, INK2)])).pad(4, 12, 4, 12)
		c.bg = Color(90 / 255.0, 29 / 255.0, 157 / 255.0, 0.25)
		c.ring = Color(242 / 255.0, 91 / 255.0, 219 / 255.0, 0.4)
		c.custom_minimum_size.y = 32
		chips.add_child(c)
	for f in [["Role", body("Ambush mesopredator", t2, INK)],
			["Body", body("1.0 m long %s 0.45 m tall" % D, t2, INK)],
			["Hunts", body("Long stillness, then a brief strike", t2, INK)],
			["Eats", chips]]:
		var dt_label := chrome(f[0], lab)
		dt_label.size_flags_vertical = Control.SIZE_SHRINK_BEGIN
		var dt := box(dt_label).pad(16, 0, 13, 0)
		dt.custom_minimum_size.x = 130
		dt.dash_top = LINE
		var dd := box(f[1]).pad(13, 0, 13, 0)
		dd.dash_top = LINE
		dd.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		facts.add_child(dt)
		facts.add_child(dd)
	col.add_child(facts)
	col.add_child(grow(spacer(), false))
	var primary := box(hbox(12, [keycap("Y", Color.WHITE), chrome("Show in world", Color.WHITE)])).pad(0, 26, 0, 14)
	primary.custom_minimum_size.y = 54
	primary.bg = Color("#8A2BA6")
	primary.inset_hi = Color("#C95BE0")
	primary.inset_lo = Color("#5A1D9D")
	primary.ring = HOT
	primary.glow = Color(176 / 255.0, 55 / 255.0, 196 / 255.0, 0.55)
	primary.outline = ACCENT
	var second := box(hbox(12, [keycap("X", INK2), chrome("Populations", INK2)])).pad(0, 26, 0, 14)
	second.custom_minimum_size.y = 54
	second.ring = Color(165 / 255.0, 149 / 255.0, 216 / 255.0, 0.45)
	col.add_child(hbox(16, [primary, second]))
	return col


func build_hints() -> void:
	var hints := hbox(36, [
			hbox(10, [keycap("A", INK2), chrome("Select", INK2)]),
			hbox(10, [keycap("B", INK2), chrome("Back", INK2)]),
			hbox(10, [keycap("LB", INK2), keycap("RB", INK2), chrome("Switch tab", INK2)])])
	hints.alignment = BoxContainer.ALIGNMENT_END
	var holder := vbox(0, [hints])
	holder.alignment = BoxContainer.ALIGNMENT_END
	holder.position = Vector2(96, 0)
	holder.size = Vector2(1728, H - 34)
	page.add_child(holder)


func paint_plate(ci: CanvasItem, s: Vector2) -> void:
	# banded sun with cut stripes, as round 1's CSS mask
	var bands := [[0.22, Color("#FDA243")], [0.38, Color("#F98C78")], [0.58, Color("#F25BDB")],
			[0.78, Color("#B037C4")], [1.0, Color("#5A1D9D")]]
	var gaps := [[0.52, 0.56], [0.64, 0.69], [0.76, 0.82], [0.88, 1.0]]
	var r := 150.0
	for y in 300:
		var f := (y + 0.5) / 300.0
		var cut := false
		for g in gaps:
			if f >= g[0] and f < g[1]:
				cut = true
		if cut:
			continue
		var c := Color.WHITE
		for b in bands:
			if f < b[0]:
				c = b[1]
				break
		c.a = 0.55
		var dy := y + 0.5 - r
		var half := sqrt(maxf(0.0, r * r - dy * dy))
		ci.draw_rect(Rect2(roundf(s.x / 2.0 - half), 70 + y, roundf(2.0 * half), 1), c)
	# horizon lines fading in towards the bottom
	var top := s.y - 130.0
	var ly := 0
	while ly < 130:
		for k in 3:
			ci.draw_rect(Rect2(0, top + ly + k, s.x, 1), Color(HOT, 0.45 * (ly + k + 0.5) / 130.0))
		ly += 24


func paint_bars(ci: CanvasItem, s: Vector2) -> void:
	var n := SERIES.size()
	var bw := clampf(floorf((s.x - 3.0 * (n - 1)) / n), 3.0, 6.0)
	var x := s.x - (n * bw + (n - 1) * 3.0)
	for i in n:
		var h: float = (SERIES[i] - 2) * 8.0
		ci.draw_rect(Rect2(x, s.y - h, bw, h), Color(ACCENT, 1.0 if i == n - 1 else 0.55))
		x += bw + 3.0


# ---------------------------------------------------------------- keys

func _unhandled_key_input(event: InputEvent) -> void:
	var k := event as InputEventKey
	if k == null or not k.pressed or k.echo:
		return
	match k.keycode:
		KEY_ESCAPE:
			get_tree().quit()
			return
		KEY_F11:
			var win := get_window()
			win.mode = Window.MODE_WINDOWED if win.mode == Window.MODE_FULLSCREEN else Window.MODE_FULLSCREEN
			return
		KEY_T:
			text_key = TEXT_ORDER[(TEXT_ORDER.find(text_key) + 1) % TEXT_ORDER.size()]
			text_step = 0
		KEY_BRACKETLEFT:
			if int(TEXTS[text_key]["grid"]) > 0:
				text_step -= 1
			else:
				text_px = maxi(12, text_px - 1)
		KEY_BRACKETRIGHT:
			if int(TEXTS[text_key]["grid"]) > 0:
				text_step += 1
			else:
				text_px += 1
		KEY_W:
			text_wght = 400 if text_wght >= 700 else text_wght + 100
		KEY_A:
			text_aa = {"auto": "gray", "gray": "none", "none": "auto"}.get(text_aa, "auto")
		KEY_H:
			text_hint = {"auto": "none", "none": "light", "light": "normal", "normal": "auto"}.get(text_hint, "auto")
		KEY_C:
			bright = not bright
		KEY_I:
			info = not info
		_:
			return
	rebuild()
