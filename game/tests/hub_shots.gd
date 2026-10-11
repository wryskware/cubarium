extends SceneTree
## Pictures of the start-up hub (main.tscn over an empty saves dir) at the window size, for
## the chrome dot rendering. Runs on a private display, never on Wrysk's screen:
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --gpu-index 1 --resolution 1920x1080 \
##       --script res://tests/hub_shots.gd -- --out DIR --sizes 1920x1080 [--dots VARIANT]
## --dots draws Gridlite's unlit dots another way (tests/dot_variants.gd).
## Writes DIR/hub-WxH.png and prints the UI scale and the title label's transform. One size per
## run: an X11 window resized in place keeps its old framebuffer for the capture.
var out := ""
var sizes := "1920x1080"
var dots := ""

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i] == "--out": out = args[i + 1]
		if args[i] == "--sizes": sizes = args[i + 1]
		if args[i] == "--dots": dots = args[i + 1]
	run()

func run() -> void:
	DirAccess.make_dir_recursive_absolute(out)
	preload("res://tests/dot_variants.gd").apply_neon(dots)
	var saves := out.path_join("hub-saves")
	preload("res://ui/saves.gd").remove_tree(saves)
	var main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	root.add_child(main)
	for s in sizes.split(","):
		var wh := s.split("x")
		DisplayServer.window_set_size(Vector2i(int(wh[0]), int(wh[1])))
		for i in 20:
			await process_frame
		await RenderingServer.frame_post_draw
		var img := root.get_texture().get_image()
		var path := out.path_join("hub-%s.png" % s)
		img.save_png(path)
		print("SHOT ", path, " window=", DisplayServer.window_get_size(), " img=", img.get_size(), " ui.scale=", main.ui.scale.x)
		for l in main.menu.find_children("*", "Label", true, false):
			if l.is_visible_in_tree():
				print("  LBL ", l.text.strip_edges(), " xform.o=", l.get_global_transform_with_canvas().origin, " size=", l.size * main.ui.scale.x)
				break
	quit(0)
