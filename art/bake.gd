extends SceneTree
## CPU bake of the actual Sprite2D pivot rigs and AnimationPlayer tracks.
## Works without a display/GPU. Every exported texel is one native cube pixel.

const NAMES = ["lantern", "sail", "mossback", "skimmer"]
const MODES = ["rest", "move", "feed", "bud"]
const HABITAT = ["rosette", "fern", "lichen"]
## Plants: name and band. Each scene carries stage0/stage1/stage2 (and optionally fruit).
const PLANTS = [["glowcap", "soil"], ["rootveil", "soil"], ["lanternstalk", "foliage"], ["tendrilfan", "foliage"], ["umbrellafrond", "canopy"], ["bloomcrown", "canopy"], ["reedspire", "water"]]
const STAGES = ["stage0", "stage1", "stage2"]
## A plant scene may also carry authored growth clips named grow<from><to> (pack v5), one
## stage step up and nonlooping: they are baked as extra rows after that plant's own rows.
const GROW_PREFIX = "grow"
## Samples per plant/tall clip (pack v4: dense so the runtime can blend adjacent samples;
## a 3 s loop is sampled every 125 ms).
const PLANT_FRAMES = 24
## Tall plants: name and the column parts the scene provides as looping clips. A trunk is
## a 4-px-periodic segment stacked every cell; base and crown cap the column.
const TALL = [["spiretree", ["base", "trunk", "crown"]], ["glasscane", ["base", "trunk", "crown"]], ["vinecoil", ["trunk"]]]
const TALL_PARTS = ["base", "trunk", "crown"]
## Ground cover: 8×8 tileable textures, four frames each, one row per band.
const GROUND = [["grit", "soil"], ["mossweave", "foliage"], ["frondmat", "canopy"]]
const GROUND_TILE = 8
const GROUND_FRAMES = 4
const GROUND_SECONDS = 6.0
const TILE = 16
## Samples per creature clip (pack v4: 16, so a 1.6 s walk is sampled every 100 ms).
const FRAMES = 16
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
	if rig.get_meta("sail_fin_coverage4", false):
		return raster_sail(rig)
	return raster_point(rig)

func raster_sail(rig: Node2D) -> Image:
	# Explicit source opt-in, not a species-wide/default filter. This compositor
	# requires the two fins below the crisp body/bud; reject contract drift.
	var paths := ["LeftFin/Sprite", "RightFin/Sprite", "Body/Sprite", "Bud/Sprite"]
	var sources := ["res://parts/sail_fin.svg", "res://parts/sail_fin.svg", "res://parts/sail_body.svg", "res://parts/bud.svg"]
	var sprites: Array[Sprite2D] = []
	collect(rig, sprites)
	if sprites.size() != paths.size():
		fail("Sail fin coverage requires exactly four authored layers")
		return raster_point(rig)
	for i in range(sprites.size()):
		if str(rig.get_path_to(sprites[i])) != paths[i] or sprites[i].texture.resource_path != sources[i]:
			fail("Sail fin coverage layer/source mismatch at " + str(i))
			return raster_point(rig)
	var visible: Array = sprites.map(func(s): return s.visible)
	sprites[2].visible = false
	sprites[3].visible = false
	var fins: Image = preload("res://sail_fin_coverage.gd").raster(rig, self)
	sprites[2].visible = visible[2]
	sprites[3].visible = visible[3]
	sprites[0].visible = false
	sprites[1].visible = false
	var crisp := raster_point(rig)
	for i in range(sprites.size()): sprites[i].visible = visible[i]
	for y in range(TILE):
		for x in range(TILE):
			var pixel := crisp.get_pixel(x,y)
			if pixel.a == 1.0: fins.set_pixel(x,y,pixel)
			elif pixel.a > 0.0: fins.set_pixel(x,y,fins.get_pixel(x,y).blend(pixel))
	return fins

func raster_point(rig: Node2D) -> Image:
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
				# A rotated part's rect test can pass a point whose texel index rounds a
				# hair outside the source; never read outside it.
				if sx < 0 or sy < 0 or sx >= source.get_width() or sy >= source.get_height():
					continue
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
	# Plants: every stage clip (and a fruit clip where the scene has one) becomes one
	# atlas row of PLANT_FRAMES frames sampled at phase frame / PLANT_FRAMES.
	var plant_rows: Array = []
	var plant_tiles: Array = []
	for entry in PLANTS:
		var plant_name: String = entry[0]
		var scene: PackedScene = load("res://plants/" + plant_name + ".tscn")
		if scene == null:
			fail("Missing plant scene " + plant_name)
			continue
		var rig := scene.instantiate() as Node2D
		root.add_child(rig)
		var player := rig.get_node("AnimationPlayer") as AnimationPlayer
		player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
		var clip_names: Array = STAGES.duplicate()
		if player.has_animation("fruit"):
			clip_names.append("fruit")
		for clip_index in range(clip_names.size()):
			var clip_name: String = clip_names[clip_index]
			if not player.has_animation(clip_name):
				fail("Missing " + clip_name + " on " + plant_name)
				continue
			var animation := player.get_animation(clip_name)
			if animation.loop_mode == Animation.LOOP_NONE:
				fail(plant_name + " " + clip_name + " must loop")
			for frame in range(PLANT_FRAMES):
				player.play("RESET")
				player.advance(0)
				player.play(clip_name)
				player.seek(float(frame) / PLANT_FRAMES * animation.length, true)
				plant_tiles.append(raster(rig))
			var stage = clip_index if clip_index < STAGES.size() else "fruit"
			plant_rows.append({"name": plant_name, "band": entry[1], "stage": stage, "row": plant_rows.size(), "frames": PLANT_FRAMES, "seconds": animation.length})
		# Growth transitions, after this plant's stage and fruit rows so the atlas stays
		# plant-major. A transition is played once, so its PLANT_FRAMES samples are spaced
		# *inclusively*: frame 0 is the source stage's neutral pose and the last frame the
		# target's, and the runtime never wraps the last sample back into the first.
		var grow_names: Array = []
		for animation_name in player.get_animation_list():
			if String(animation_name).begins_with(GROW_PREFIX):
				grow_names.append(String(animation_name))
		grow_names.sort()
		for grow_name in grow_names:
			var steps: String = grow_name.substr(GROW_PREFIX.length())
			# Validate each stage digit: is_valid_int() on the whole suffix also accepts
			# signed values such as "+1", which would otherwise be misread as grow01.
			if steps.length() != 2 or not steps.substr(0, 1).is_valid_int() or not steps.substr(1, 1).is_valid_int():
				fail(plant_name + ": " + grow_name + " must be named grow<from><to>")
				continue
			var from: int = steps.substr(0, 1).to_int()
			var to: int = steps.substr(1, 1).to_int()
			if from + 1 != to or to > STAGES.size() - 1:
				fail(plant_name + " " + grow_name + " must go one stage step up")
				continue
			var growth := player.get_animation(grow_name)
			if growth.loop_mode != Animation.LOOP_NONE:
				fail(plant_name + " " + grow_name + " must not loop")
				continue
			for frame in range(PLANT_FRAMES):
				player.play("RESET")
				player.advance(0)
				player.play(grow_name)
				player.seek(float(frame) / (PLANT_FRAMES - 1) * growth.length, true)
				plant_tiles.append(raster(rig))
			plant_rows.append({"name": plant_name, "band": entry[1], "stage": "grow", "from": from, "to": to, "row": plant_rows.size(), "frames": PLANT_FRAMES, "seconds": growth.length, "loop": false})
		rig.queue_free()
	var plant_atlas := Image.create(TILE * PLANT_FRAMES, TILE * maxi(plant_rows.size(), 1), false, Image.FORMAT_RGBA8)
	plant_atlas.fill(Color.TRANSPARENT)
	for i in range(plant_tiles.size()):
		plant_atlas.blit_rect(plant_tiles[i], Rect2i(0, 0, TILE, TILE), Vector2i((i % PLANT_FRAMES) * TILE, (i / PLANT_FRAMES) * TILE))
	# Tall plants: every column part clip becomes one atlas row of PLANT_FRAMES frames.
	var tall_rows: Array = []
	var tall_tiles: Array = []
	for entry in TALL:
		var tall_name: String = entry[0]
		var scene: PackedScene = load("res://plants/" + tall_name + ".tscn")
		if scene == null:
			fail("Missing tall plant scene " + tall_name)
			continue
		var rig := scene.instantiate() as Node2D
		root.add_child(rig)
		var player := rig.get_node("AnimationPlayer") as AnimationPlayer
		player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
		for part_name in TALL_PARTS:
			var wanted: bool = entry[1].has(part_name)
			if player.has_animation(part_name) != wanted:
				fail(tall_name + ": clip " + part_name + (" missing" if wanted else " not declared in TALL"))
				continue
			if not wanted:
				continue
			var animation := player.get_animation(part_name)
			if animation.loop_mode == Animation.LOOP_NONE:
				fail(tall_name + " " + part_name + " must loop")
			for frame in range(PLANT_FRAMES):
				player.play("RESET")
				player.advance(0)
				player.play(part_name)
				player.seek(float(frame) / PLANT_FRAMES * animation.length, true)
				tall_tiles.append(raster(rig))
			var tall_row := {"name": tall_name, "part": part_name, "row": tall_rows.size(), "frames": PLANT_FRAMES, "seconds": animation.length}
			# Additive opt-in only: keep every authored atlas pixel intact for old readers.
			if tall_name == "vinecoil" and part_name == "trunk":
				tall_row["vine_strips"] = "period4_endpoint_v1"
			tall_rows.append(tall_row)
		rig.queue_free()
	var tall_atlas := Image.create(TILE * PLANT_FRAMES, TILE * maxi(tall_rows.size(), 1), false, Image.FORMAT_RGBA8)
	tall_atlas.fill(Color.TRANSPARENT)
	for i in range(tall_tiles.size()):
		tall_atlas.blit_rect(tall_tiles[i], Rect2i(0, 0, TILE, TILE), Vector2i((i % PLANT_FRAMES) * TILE, (i / PLANT_FRAMES) * TILE))
	# Ground cover: authored frame SVGs blitted as they are (no rig, no transform).
	var ground_rows: Array = []
	var ground_atlas := Image.create(GROUND_TILE * GROUND_FRAMES, GROUND_TILE * GROUND.size(), false, Image.FORMAT_RGBA8)
	ground_atlas.fill(Color.TRANSPARENT)
	for g in range(GROUND.size()):
		for frame in range(GROUND_FRAMES):
			var source := Image.new()
			var path: String = "res://ground/" + GROUND[g][0] + "_" + str(frame) + ".svg"
			if source.load(ProjectSettings.globalize_path(path)) != OK:
				fail("Cannot read ground source " + path)
				continue
			source.convert(Image.FORMAT_RGBA8)
			if source.get_size() != Vector2i(GROUND_TILE, GROUND_TILE):
				fail("Ground tile must be 8×8: " + path)
				continue
			ground_atlas.blit_rect(source, Rect2i(0, 0, GROUND_TILE, GROUND_TILE), Vector2i(frame * GROUND_TILE, g * GROUND_TILE))
		ground_rows.append({"name": GROUND[g][0], "band": GROUND[g][1], "row": g, "frames": GROUND_FRAMES, "seconds": GROUND_SECONDS})
	if failed:
		quit(1)
		return
	if atlas.save_png(output + "/creatures.png") != OK or plants.save_png(output + "/habitat.png") != OK or plant_atlas.save_png(output + "/plants.png") != OK or tall_atlas.save_png(output + "/tall.png") != OK or ground_atlas.save_png(output + "/ground.png") != OK:
		fail("Cannot write atlas PNGs")
	var file := FileAccess.open(output + "/pack.json", FileAccess.WRITE)
	if file == null:
		fail("Cannot write pack.json")
	else:
		file.store_string(JSON.stringify({"version": 5, "tile": TILE, "frames": FRAMES, "pivot": [8,8], "facing": "+x", "creatures": "creatures.png", "habitat": "habitat.png", "habitat_names": HABITAT, "creature_names": NAMES, "clips": clips, "plant_atlas": "plants.png", "plant_frames": PLANT_FRAMES, "plants": plant_rows, "tall_atlas": "tall.png", "tall": tall_rows, "ground_atlas": "ground.png", "ground_tile": GROUND_TILE, "ground_frames": GROUND_FRAMES, "ground": ground_rows}, "\t") + "\n")
	print("Cubarium: baked ", NAMES.size(), " rigs, ", clips.size(), " clips, three habitat motifs, ", plant_rows.size(), " plant rows, ", tall_rows.size(), " tall rows and ", ground_rows.size(), " ground tiles to ", output)
	quit(1 if failed else 0)
