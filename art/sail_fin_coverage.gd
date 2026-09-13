extends RefCounted
## Sail-only offline coverage kernel, copied from the independently measured study.
## Caller selects the two lower fin layers; body/bud remain point sampled.
const TILE := 16
const GRID := 4


static func raster(rig: Node2D, baker: SceneTree) -> Image:
	# Quantize each subpixel's source-over layers just as the existing RGBA8 bake does.
	# Then resolve spatial coverage in linear premultiplied RGBA, not encoded RGB.
	var fine := Image.create(TILE * GRID, TILE * GRID, false, Image.FORMAT_RGBA8)
	fine.fill(Color.TRANSPARENT)
	var sprites: Array[Sprite2D] = []
	baker.collect(rig, sprites)
	for sprite in sprites:
		if sprite.z_index != 0 or sprite.region_enabled or sprite.hframes != 1 or sprite.vframes != 1:
			baker.fail("Unsupported sprite setting " + str(sprite.get_path()))
			continue
		if not sprite.is_visible_in_tree(): continue
		var source: Image = baker.source_image(sprite)
		if source.get_size() != Vector2i(sprite.texture.get_size()):
			baker.fail("Source/import size mismatch " + str(sprite.get_path()))
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
			var constant := true
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
