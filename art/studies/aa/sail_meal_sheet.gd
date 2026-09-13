extends SceneTree
## Deterministic native crop review of the first actual sail bout after the care boundary.
## This is a screenshot contact sheet, not a new sprite or retouched image.
func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 1, "CAPTURE_DIRECTORY")
	var dir: String = args[0]
	var rows: Array = JSON.parse_string(FileAccess.get_file_as_string(dir.path_join("sails.json")))
	var bouts := rows.filter(func(r): return r.elapsed >= 600 and r.meal != null and r.meal.weight_prev == 0 and r.meal.weight > 0)
	assert(not bouts.is_empty(), "No actual sail meal onset after care boundary")
	var first: Dictionary = bouts[0]
	var offsets := [-1, 0, 1, 3, 6, 9, 15, 30]
	var sheet := Image.create(256, 128 * offsets.size(), false, Image.FORMAT_RGB8)
	sheet.fill(Color.BLACK)
	var selected: Array = []
	for index in range(offsets.size()):
		var frame: int = int(first.elapsed) * 3 + offsets[index]
		var elapsed: int = frame / 3
		var current := rows.filter(func(r): return int(r.elapsed) == elapsed and r.slot == first.slot and r.generation == first.generation)
		assert(current.size() == 1)
		var r: Dictionary = current[0]
		var origin: Vector2i = [Vector2i(64,64),Vector2i(128,64),Vector2i(192,64),Vector2i(0,64),Vector2i(64,0)][int(r.face)]
		origin += Vector2i(clampi(int(floor(r.u))-12,0,40),clampi(int(floor(r.v))-12,0,40))
		for mode in range(2):
			var label: String = ["old", "new"][mode]
			var image := Image.load_from_file(dir.path_join(label).path_join("frame_%05d.png" % frame))
			var crop := image.get_region(Rect2i(origin,Vector2i(24,24)))
			sheet.blit_rect(crop,Rect2i(0,0,24,24),Vector2i(mode*128,index*128))
			crop.resize(96,96,Image.INTERPOLATE_NEAREST)
			sheet.blit_rect(crop,Rect2i(0,0,96,96),Vector2i(mode*128,index*128+28))
		selected.append({"frame":frame,"row":r})
	assert(sheet.save_png(dir.path_join("meal-contact.png")) == OK)
	var report := FileAccess.open(dir.path_join("meal-contact.json"),FileAccess.WRITE)
	report.store_string(JSON.stringify({"selection":"First actual sail meal onset at/after elapsed600; no forced form or action","columns":["original","stable-body-plus-fin4"],"row_frame_offsets":offsets,"selected":selected,"crop":"24px face-clipped neighborhood, native plus nearest4x; not seam-unfolded"},"  "))
	quit()
