extends SceneTree
## Measures the light Gridlite's own dots carry as Godot renders them today (the game's
## font import: gray antialiasing, light hinting, no subpixel positioning), per zone and
## axis value, at the chrome (26 px, ELSH 2.65) and title (91 px, ELSH 3.5) sizes. Each
## cell is "AAAA" in white on black with one zone on and the others at 1; the light per
## dot is the summed coverage over the label divided by that zone's dot count (Gridlite's
## "A": 18 lit, 31 BACK and 42 RECT dots), in px² of full colour. Prints a table and, with
## --out FILE, writes it as JSON. Needs a renderer, so it runs on a private display:
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --gpu-index 1 \
##       --resolution 1920x1080 --script res://tests/gridlite_light.gd -- [--out FILE]

const ROLES := {
	"chrome": {"size": 26, "elsh": 2.65, "baseline": -0.0765},
	"title": {"size": 91, "elsh": 3.5, "baseline": -0.038},
}
const VALUES := [30.0, 71.0, 90.0, 120.0, 135.0, 180.0, 220.0, 240.0, 320.0, 450.0, 600.0, 691.0, 700.0, 900.0]
## Dots of each zone in one "A" (7 columns × 13 rows).
const PER_A := {"wght": 18, "BACK": 31, "RECT": 42}
const TEXT := "AAAA"

var out := ""


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i] == "--out": out = args[i + 1]
	run()


func _font(role: Dictionary, zone: String, v: float) -> FontVariation:
	var ts := TextServerManager.get_primary_interface()
	var fv := FontVariation.new()
	fv.base_font = load("res://fonts/GridlitePEVFTRIAL-All.ttf")
	var ax := {"wght": 1.0, "BACK": 1.0, "RECT": 1.0, "ELSH": role.elsh}
	ax[zone] = v
	var ot := {}
	for t in ax:
		ot[ts.name_to_tag(t)] = ax[t]
	fv.variation_opentype = ot
	fv.baseline_offset = role.baseline
	return fv


func run() -> void:
	DisplayServer.window_set_size(Vector2i(1920, 1080))
	var bg := ColorRect.new()
	bg.color = Color.BLACK
	bg.size = Vector2(1920, 1080)
	root.add_child(bg)
	var rows := []
	for role_name in ROLES:
		var role: Dictionary = ROLES[role_name]
		for zone in PER_A:
			for v in VALUES:
				rows.append({"role": role_name, "zone": zone, "value": v})
	# lay the cells out in batches that fit the window, one frame each
	var results := []
	var i := 0
	while i < rows.size():
		var labels := []
		var x := 10
		var y := 10
		var line_h := 0
		while i < rows.size():
			var r: Dictionary = rows[i]
			var role: Dictionary = ROLES[r.role]
			var l := Label.new()
			l.add_theme_font_override("font", _font(role, r.zone, r.value))
			l.add_theme_font_size_override("font_size", role.size)
			l.add_theme_color_override("font_color", Color.WHITE)
			l.text = TEXT
			# Gridlite's advance is 7 dots (2800 of 5200 units); the line box is the em
			var w := int(ceil(TEXT.length() * 2800.0 * role.size / 5200.0)) + 40
			var h: int = role.size + 40
			if x + w > 1910:
				x = 10
				y += line_h
				line_h = 0
			if y + h > 1070:
				l.free()
				break
			l.position = Vector2(x + 20, y + 20)
			bg.add_child(l)
			labels.append({"label": l, "rect": Rect2i(x, y, w, h), "row": r})
			x += w
			line_h = maxi(line_h, h)
			i += 1
		for k in 6:
			await process_frame
		await RenderingServer.frame_post_draw
		var img := root.get_texture().get_image()
		if out != "":
			img.save_png(out.get_basename() + "-batch%d.png" % i)
		for e in labels:
			var rect: Rect2i = e.rect
			var sum := 0.0
			for py in range(rect.position.y, rect.end.y):
				for px in range(rect.position.x, rect.end.x):
					sum += img.get_pixel(px, py).r
			var r: Dictionary = e.row
			var n: int = PER_A[r.zone] * TEXT.length()
			results.append({"role": r.role, "zone": r.zone, "value": r.value, "light": sum / n})
			e.label.queue_free()
		await process_frame
	print("role    zone  value   light/dot (px²)")
	for r in results:
		print("%-7s %-5s %5.0f   %.3f" % [r.role, r.zone, r.value, r.light])
	if out != "":
		var f := FileAccess.open(out, FileAccess.WRITE)
		f.store_string(JSON.stringify(results, "  "))
	quit(0)
