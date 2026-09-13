extends "res://bake.gd"
## Glasscane wind-room candidate (2026-09-13): the registered clear-end-rows rule
## (`art_present::trunk_strip`, first used by the spiretree) applied to the glasscane
## trunk's tile rows 0 and 15, plus the crown's trunk-pattern row 15 (otherwise the
## loader's derived cap would keep it, since a cleared trunk no longer paints it, and the
## cap would be bound at 0.59 px by that row instead of 1.25 by the side bulbs).
## Everything else — the three lantern bulbs, the magenta joints, the root-glass base,
## every other species, clip timing, pack metadata — is untouched. The rows are cleared
## on cached image COPIES at bake time; no source SVG, existing output or production
## atlas is edited. The equivalent source edit is recorded by `author_candidate.py`.
const TRUNK := "res://parts/plant_glasscane_trunk.svg"
const CROWN := "res://parts/plant_glasscane_crown.svg"
var cleared: Dictionary = {}

func source_image(sprite: Sprite2D) -> Image:
	var source := super.source_image(sprite)
	var path := sprite.texture.resource_path
	var rows: Array = []
	if path == TRUNK:
		rows = [0, 15]
	elif path == CROWN:
		rows = [15]
	else:
		return source
	if not cleared.has(path):
		var copy := source.duplicate() as Image
		assert(copy.get_height() == 16, path + " is not a 16-row part")
		for y in rows:
			for x in range(copy.get_width()):
				copy.set_pixel(x, y, Color.TRANSPARENT)
		cleared[path] = copy
	return cleared[path]

func bake() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="):
			output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output):
		push_error("Glasscane wind study requires a NEW --out directory")
		quit(1)
		return
	super.bake()
