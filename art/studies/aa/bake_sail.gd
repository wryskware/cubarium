extends "res://studies/aa/bake_comparison.gd"
## One fixed candidate: the two known fin layers receive area coverage. Body and bud
## retain current pixel-center sampling, including all their authored scale/visibility.
const PATHS = ["LeftFin/Sprite", "RightFin/Sprite", "Body/Sprite", "Bud/Sprite"]
const SOURCES = ["res://parts/sail_fin.svg", "res://parts/sail_fin.svg", "res://parts/sail_body.svg", "res://parts/bud.svg"]

func preserve_constant_blocks() -> bool:
	# No spatial mixture means no resolve: keep exact authored RGBA codes rather than
	# losing an occasional code through floating conversion followed by RGBA8 truncation.
	return true

func coverage(rig: Node2D) -> Image:
	var sprites: Array[Sprite2D] = []
	collect(rig, sprites)
	var visible: Array = sprites.map(func(s): return s.visible)
	# Resolve only the lower fin layers, then apply the original point-sampled body/bud
	# above them. This avoids even a color-space round trip on opaque body highlights.
	sprites[2].visible = false
	sprites[3].visible = false
	var fins := super.coverage(rig)
	sprites[2].visible = visible[2]
	sprites[3].visible = visible[3]
	sprites[0].visible = false
	sprites[1].visible = false
	var crisp := raster(rig)
	for i in range(sprites.size()): sprites[i].visible = visible[i]
	for y in range(TILE):
		for x in range(TILE):
			var pixel := crisp.get_pixel(x,y)
			if pixel.a == 1.0: fins.set_pixel(x,y,pixel)
			elif pixel.a > 0.0: fins.set_pixel(x,y,fins.get_pixel(x,y).blend(pixel))
	return fins

func same_visible(a: Image, b: Image) -> bool:
	for y in range(a.get_height()):
		for x in range(a.get_width()):
			var p := a.get_pixel(x,y)
			var q := b.get_pixel(x,y)
			if p.a != q.a or (p.a > 0.0 and p != q): return false
	return true

func select_pose(player: AnimationPlayer, mode: String, seconds: float) -> void:
	player.play("RESET")
	player.advance(0)
	player.play(mode)
	player.seek(seconds, true)

func study() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output) or DirAccess.make_dir_recursive_absolute(output) != OK:
		push_error("Sail study requires a writable NEW --out directory")
		quit(1)
		return
	var rig := (load("res://creatures/sail.tscn") as PackedScene).instantiate() as Node2D
	root.add_child(rig)
	if rig.has_meta("sail_fin_coverage4"):
		push_error("Historical fin-only study requires the original sail rig (8c0b40a). Use bake_sail_stable.gd for the stable-body candidate.")
		quit(1)
		return
	var sprites: Array[Sprite2D] = []
	collect(rig, sprites)
	if sprites.size() != PATHS.size(): fail("Unexpected sail layer count")
	for i in range(mini(sprites.size(), PATHS.size())):
		if str(rig.get_path_to(sprites[i])) != PATHS[i] or sprites[i].texture.resource_path != SOURCES[i]:
			fail("Sail layer contract changed at index " + str(i))
	if failed:
		quit(1)
		return
	var shipped := Image.load_from_file(ProjectSettings.globalize_path("res://../assets/atelier/creatures.png"))
	shipped.convert(Image.FORMAT_RGBA8)
	var player := rig.get_node("AnimationPlayer") as AnimationPlayer
	player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
	var metadata: Array = []
	for mode_index in range(MODES.size()):
		var mode: String = MODES[mode_index]
		var clip := player.get_animation(mode)
		var baseline := Image.create(TILE * FRAMES, TILE, false, Image.FORMAT_RGBA8)
		var candidate := Image.create(TILE * FRAMES, TILE, false, Image.FORMAT_RGBA8)
		var protected := 0
		var body_alpha: Array = []
		var timing := [0, 0]
		for frame in range(FRAMES):
			select_pose(player, mode, float(frame) / (FRAMES - 1 if mode == "bud" else FRAMES) * clip.length)
			var began := Time.get_ticks_usec()
			var point := raster(rig)
			timing[0] += Time.get_ticks_usec() - began
			began = Time.get_ticks_usec()
			var fin4 := coverage(rig)
			timing[1] += Time.get_ticks_usec() - began
			baseline.blit_rect(point, Rect2i(0, 0, TILE, TILE), Vector2i(frame * TILE, 0))
			candidate.blit_rect(fin4, Rect2i(0, 0, TILE, TILE), Vector2i(frame * TILE, 0))
			# With the two fins hidden, point and candidate must be exactly identical.
			# Every opaque overlying body/bud pixel also remains exact in the full composite.
			sprites[0].visible = false
			sprites[1].visible = false
			var crisp := raster(rig)
			var alpha_sum := 0.0
			if not same_visible(crisp, coverage(rig)): fail(mode + ": non-fin layer changed")
			for y in range(TILE):
				for x in range(TILE):
					alpha_sum += crisp.get_pixel(x,y).a
					if crisp.get_pixel(x,y).a == 1.0:
						protected += 1
						if point.get_pixel(x,y) != fin4.get_pixel(x,y): fail(mode + ": opaque body pixel changed")
			body_alpha.append(alpha_sum)
			sprites[0].visible = true
			sprites[1].visible = true
		if baseline.get_data() != shipped.get_region(Rect2i(0, (4 + mode_index) * TILE, TILE * FRAMES, TILE)).get_data():
			fail(mode + ": baseline differs from shipped atlas")
		# Verify the exporter endpoints separately from runtime bracketing tests.
		select_pose(player, mode, clip.length)
		var endpoint := coverage(rig)
		var endpoint_frame: int = FRAMES - 1 if mode == "bud" else 0
		if endpoint.get_data() != candidate.get_region(Rect2i(endpoint_frame * TILE,0,TILE,TILE)).get_data():
			fail(mode + ": authored endpoint mismatch")
		if baseline.save_png(output.path_join("sail-" + mode + "-nearest.png")) != OK or candidate.save_png(output.path_join("sail-" + mode + "-fin4.png")) != OK:
			fail("Cannot save " + mode)
		metadata.append({"name":"sail-"+mode,"clip":mode,"seconds":clip.length,"loop":mode!="bud","frames":FRAMES,
			"point_body_exact":true,"protected_opaque_pixels":protected,"shipped_baseline_exact":true,
			"point_body_and_bud_alpha_by_frame":body_alpha,"authored_endpoint_exact":true,"bake_us_nearest":timing[0],"bake_us_fin4":timing[1]})
	var file := FileAccess.open(output.path_join("cases.json"), FileAccess.WRITE)
	if file == null:
		push_error("Cannot save metadata")
		quit(1)
		return
	file.store_string(JSON.stringify({"passed":not failed,"coverage_layers":PATHS.slice(0,2),"point_layers":PATHS.slice(2),"cases":metadata},"  "))
	print(JSON.stringify(metadata))
	quit(1 if failed else 0)
