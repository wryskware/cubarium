extends "res://bake.gd"
## Isolated coverage experiment. Never writes the production atlas.
## Same rigs, sample times and cutout compositor; only spatial coverage is integrated.
const GRID := 4
const CASES = [
	["lanternstalk", "plants", "stage1", 24],
	["reedspire", "plants", "stage1", 24],
	["sail", "creatures", "move", 16],
	["skimmer", "creatures", "move", 16],
]

func _initialize() -> void:
	call_deferred("study")

func preserve_constant_blocks() -> bool:
	return false

func coverage(rig: Node2D) -> Image:
	# Quantize each subpixel's source-over layers just as the existing RGBA8 bake does.
	# Then resolve spatial coverage in linear premultiplied RGBA, not encoded RGB.
	var fine := Image.create(TILE * GRID, TILE * GRID, false, Image.FORMAT_RGBA8)
	fine.fill(Color.TRANSPARENT)
	var sprites: Array[Sprite2D] = []
	collect(rig, sprites)
	for sprite in sprites:
		if sprite.z_index != 0 or sprite.region_enabled or sprite.hframes != 1 or sprite.vframes != 1:
			fail("Unsupported sprite setting " + str(sprite.get_path()))
			continue
		if not sprite.is_visible_in_tree(): continue
		var source := source_image(sprite)
		if source.get_size() != Vector2i(sprite.texture.get_size()):
			fail("Source/import size mismatch " + str(sprite.get_path()))
			continue
		var transform := rig.global_transform.affine_inverse() * sprite.global_transform
		if absf(transform.determinant()) < 0.000001: continue
		var inverse := transform.affine_inverse()
		var rect := sprite.get_rect()
		for y in range(TILE * GRID):
			for x in range(TILE * GRID):
				var point := Vector2((x + 0.5) / GRID - TILE / 2.0, (y + 0.5) / GRID - TILE / 2.0)
				var local := inverse * point
				if not rect.has_point(local): continue
				var uv := local - rect.position
				var sx := int(floor(uv.x))
				var sy := int(floor(uv.y))
				if sprite.flip_h: sx = source.get_width() - 1 - sx
				if sprite.flip_v: sy = source.get_height() - 1 - sy
				if sx < 0 or sy < 0 or sx >= source.get_width() or sy >= source.get_height(): continue
				var color := source.get_pixel(sx, sy) * sprite.modulate * sprite.self_modulate
				var parent := sprite.get_parent()
				while parent is CanvasItem and parent != rig.get_parent():
					color *= parent.modulate
					parent = parent.get_parent()
				fine.set_pixel(x, y, fine.get_pixel(x, y).blend(color))
	var image := Image.create(TILE, TILE, false, Image.FORMAT_RGBA8)
	image.fill(Color.TRANSPARENT)
	for y in range(TILE):
		for x in range(TILE):
			var sum := Color(0, 0, 0, 0)
			var first := fine.get_pixel(x * GRID, y * GRID)
			var constant := preserve_constant_blocks()
			for sy in range(GRID):
				for sx in range(GRID):
					var c := fine.get_pixel(x * GRID + sx, y * GRID + sy)
					constant = constant and c == first
					var linear := c.srgb_to_linear()
					sum += Color(linear.r * c.a, linear.g * c.a, linear.b * c.a, c.a)
			if constant:
				image.set_pixel(x, y, first)
			elif sum.a > 0:
				var resolved := Color(sum.r / sum.a, sum.g / sum.a, sum.b / sum.a, sum.a / (GRID * GRID))
				image.set_pixel(x, y, resolved.linear_to_srgb())
	return image

func study() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output):
		push_error("Study requires --out=<NEW directory>")
		quit(1)
		return
	if DirAccess.make_dir_recursive_absolute(output) != OK:
		push_error("Cannot create " + output)
		quit(1)
		return
	var metadata: Array = []
	for entry in CASES:
		var rig := (load("res://" + entry[1] + "/" + entry[0] + ".tscn") as PackedScene).instantiate() as Node2D
		root.add_child(rig)
		var player := rig.get_node("AnimationPlayer") as AnimationPlayer
		player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
		var clip := player.get_animation(entry[2])
		var frames: int = entry[3]
		var baseline := Image.create(TILE * frames, TILE, false, Image.FORMAT_RGBA8)
		var candidate := Image.create(TILE * frames, TILE, false, Image.FORMAT_RGBA8)
		var micros := [0, 0]
		for frame in range(frames):
			player.play("RESET")
			player.advance(0)
			player.play(entry[2])
			player.seek(float(frame) / frames * clip.length, true)
			var began := Time.get_ticks_usec()
			baseline.blit_rect(super.raster(rig), Rect2i(0, 0, TILE, TILE), Vector2i(frame * TILE, 0))
			micros[0] += Time.get_ticks_usec() - began
			began = Time.get_ticks_usec()
			candidate.blit_rect(coverage(rig), Rect2i(0, 0, TILE, TILE), Vector2i(frame * TILE, 0))
			micros[1] += Time.get_ticks_usec() - began
		if baseline.save_png(output.path_join(entry[0] + "-nearest.png")) != OK or candidate.save_png(output.path_join(entry[0] + "-coverage4.png")) != OK:
			fail("Cannot save study atlas " + entry[0])
		metadata.append({"name":entry[0], "kind":entry[1], "clip":entry[2], "frames":frames,
			"seconds":clip.length, "bake_us_nearest":micros[0], "bake_us_coverage4":micros[1]})
		rig.queue_free()
	var file := FileAccess.open(output.path_join("cases.json"), FileAccess.WRITE)
	if file == null:
		push_error("Cannot save study metadata")
		quit(1)
		return
	file.store_string(JSON.stringify(metadata, "  "))
	print(JSON.stringify(metadata))
	quit(1 if failed else 0)
