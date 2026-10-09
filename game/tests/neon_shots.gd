extends SceneTree
## One frame of a neon UI scene, for checking it against round 3 without a window on the
## screen. Runs on a private display:
##   scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/neon_shots.gd -- --scene res://ui/neon/gallery.tscn \
##       --out /tmp/gallery.png [--focus NODE_PATH]
## --focus gives keyboard focus to the node at NODE_PATH (relative to the scene's root).

var scene := "res://ui/neon/gallery.tscn"
var out := ""
var focus := ""


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--scene": scene = args[i + 1]
			"--out": out = args[i + 1]
			"--focus": focus = args[i + 1]
	if out == "":
		out = OS.get_user_data_dir().path_join(scene.get_file().get_basename() + ".png")
	run()


func run() -> void:
	var packed: PackedScene = load(scene)
	if packed == null:
		push_error("neon_shots: can't load %s" % scene)
		quit(1)
		return
	var node := packed.instantiate()
	root.add_child(node)
	for i in 3:
		await process_frame
	if focus != "":
		var c := node.get_node_or_null(focus) as Control
		if c == null:
			push_error("neon_shots: no control at %s" % focus)
		else:
			c.grab_focus()
	for i in 4:
		await process_frame
	await RenderingServer.frame_post_draw
	var img := root.get_texture().get_image()
	var err := img.save_png(out)
	print("neon_shots: wrote %s %dx%d (%s)" % [out, img.get_width(), img.get_height(), error_string(err)])
	quit(0 if err == OK else 1)
