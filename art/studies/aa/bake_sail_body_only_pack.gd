extends "res://bake.gd"
## Ablation only: edit the loaded instance, never the source scene or production pack.
func raster(rig: Node2D) -> Image:
	if rig.get_meta("sail_fin_coverage4", false): rig.set_meta("sail_fin_coverage4", false)
	return super.raster(rig)

func bake() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output):
		push_error("Body-only ablation requires a NEW --out directory")
		quit(1)
		return
	super.bake()
