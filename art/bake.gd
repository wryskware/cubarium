extends SceneTree
## CPU bake of the actual Sprite2D pivot rigs and AnimationPlayer tracks.
## Works without a display/GPU. Every exported texel is one native cube pixel.

const NAMES = ["lantern", "sail", "mossback"]
const MODES = ["rest", "move", "feed", "bud"]
const HABITAT = ["rosette", "fern", "lichen"]
const TILE = 16
const FRAMES = 8
var images: Dictionary = {}
var failed := false

func _initialize() -> void:
	call_deferred("bake")

func fail(message: String) -> void:
	push_error(message)
	failed = true

func source_image(sprite: Sprite2D) -> Image:
	var path := sprite.texture.resource_path
	if not images.has(path):
		var image := Image.new()
		if image.load(ProjectSettings.globalize_path(path)) != OK:
			fail("Cannot read sprite source " + path)
		image.convert(Image.FORMAT_RGBA8)
		images[path] = image
	return images[path]

func collect(node: Node, result: Array[Sprite2D]) -> void:
	if node is Sprite2D:
		result.append(node)
	for child in node.get_children():
		collect(child, result)

func raster(rig: Node2D) -> Image:
	var image := Image.create(TILE, TILE, false, Image.FORMAT_RGBA8)
	image.fill(Color.TRANSPARENT)
	var sprites: Array[Sprite2D] = []
	collect(rig, sprites)
	# This cutout contract uses sibling/tree draw order, zero z_index, standard
	# source-over blend and Sprite2D transforms. Reject silently unsupported art.
	for sprite in sprites:
		if sprite.z_index != 0 or sprite.region_enabled or sprite.hframes != 1 or sprite.vframes != 1:
			fail("Unsupported sprite setting at " + str(sprite.get_path()))
			continue
		if not sprite.is_visible_in_tree():
			continue
		var source := source_image(sprite)
		if source.get_size() != Vector2i(sprite.texture.get_size()):
			fail("Source/import size mismatch at " + str(sprite.get_path()) + "; use import scale 1")
			continue
		var transform := rig.global_transform.affine_inverse() * sprite.global_transform
		if absf(transform.determinant()) < 0.000001:
			continue
		var inverse := transform.affine_inverse()
		var rect := sprite.get_rect()
		# Sample in the same pixel-centre convention as the cube renderer.
		for y in range(TILE):
			for x in range(TILE):
				var local := inverse * Vector2(x + 0.5 - TILE / 2.0, y + 0.5 - TILE / 2.0)
				if not rect.has_point(local):
					continue
				var uv := local - rect.position
				var sx := int(floor(uv.x))
				var sy := int(floor(uv.y))
				if sprite.flip_h: sx = source.get_width() - 1 - sx
				if sprite.flip_v: sy = source.get_height() - 1 - sy
				var color := source.get_pixel(sx, sy) * sprite.modulate * sprite.self_modulate
				var parent := sprite.get_parent()
				while parent is CanvasItem and parent != rig.get_parent():
					color *= parent.modulate
					parent = parent.get_parent()
				image.set_pixel(x, y, image.get_pixel(x, y).blend(color))
	return image

func bake() -> void:
	var output := ProjectSettings.globalize_path("res://../assets/atelier")
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if DirAccess.make_dir_recursive_absolute(output) != OK:
		fail("Cannot create " + output)
		quit(1)
		return
	var atlas := Image.create(TILE * FRAMES, TILE * NAMES.size() * MODES.size(), false, Image.FORMAT_RGBA8)
	atlas.fill(Color.TRANSPARENT)
	var clips: Array = []
	for species in range(NAMES.size()):
		var scene: PackedScene = load("res://creatures/" + NAMES[species] + ".tscn")
		if scene == null:
			fail("Missing creature scene")
			continue
		var rig := scene.instantiate() as Node2D
		root.add_child(rig)
		var player := rig.get_node("AnimationPlayer") as AnimationPlayer
		player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
		for mode_index in range(MODES.size()):
			var mode: String = MODES[mode_index]
			if not player.has_animation(mode):
				fail("Missing " + mode + " on " + NAMES[species])
				continue
			var animation := player.get_animation(mode)
			var row: int = species * MODES.size() + mode_index
			for frame in range(FRAMES):
				player.play("RESET")
				player.advance(0)
				player.play(mode)
				var phase: float = float(frame) / (FRAMES - 1 if mode == "bud" else FRAMES)
				player.seek(phase * animation.length, true)
				atlas.blit_rect(raster(rig), Rect2i(0, 0, TILE, TILE), Vector2i(frame * TILE, row * TILE))
			clips.append({"name": NAMES[species], "state": mode, "row": row, "seconds": animation.length, "loop": animation.loop_mode != Animation.LOOP_NONE})
		rig.queue_free()
	var plants := Image.create(TILE * HABITAT.size(), TILE, false, Image.FORMAT_RGBA8)
	plants.fill(Color.TRANSPARENT)
	for i in range(HABITAT.size()):
		var source := Image.new()
		if source.load(ProjectSettings.globalize_path("res://habitat/" + HABITAT[i] + ".svg")) != OK:
			fail("Cannot read habitat source")
			continue
		source.convert(Image.FORMAT_RGBA8)
		plants.blit_rect(source, Rect2i(Vector2i.ZERO, source.get_size()), Vector2i(i * TILE, 0) + (Vector2i(TILE, TILE) - source.get_size()) / 2)
	if failed:
		quit(1)
		return
	if atlas.save_png(output + "/creatures.png") != OK or plants.save_png(output + "/habitat.png") != OK:
		fail("Cannot write atlas PNGs")
	var file := FileAccess.open(output + "/pack.json", FileAccess.WRITE)
	if file == null:
		fail("Cannot write pack.json")
	else:
		file.store_string(JSON.stringify({"version": 1, "tile": TILE, "frames": FRAMES, "pivot": [8,8], "facing": "+x", "creatures": "creatures.png", "habitat": "habitat.png", "habitat_names": HABITAT, "clips": clips}, "\t") + "\n")
	print("Cubarium: baked three rigs, twelve clips and three habitat motifs to ", output)
	quit(1 if failed else 0)
