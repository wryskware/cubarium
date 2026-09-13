extends "res://bake.gd"
## One candidate: remove the two extreme source rows on a cached image COPY.
## No source SVG, existing output, or production atlas is edited.
func source_image(sprite: Sprite2D) -> Image:
	var source := super.source_image(sprite)
	if sprite.texture.resource_path != "res://parts/plant_vinecoil_trunk.svg": return source
	var cleared := source.duplicate() as Image
	for y in [0,15]:
		for x in range(cleared.get_width()): cleared.set_pixel(x,y,Color.TRANSPARENT)
	return cleared

func bake() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output):
		push_error("Vine study requires a NEW --out directory")
		quit(1)
		return
	super.bake()
