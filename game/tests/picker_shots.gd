extends SceneTree
## Pictures of the terrain-first new-world picker, driven without input. Needs a renderer, so
## it runs on a private display (never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/picker_shots.gd -- --out DIR [--size terrarium] [--seed 5]
## Writes DIR/picker-low.png, picker-medium.png, picker-high.png (the same terrains at each
## water step), founding-1.png and founding-2.png (the founding panel part way), and
## picker-founded.png (the world running), then quits.

var main
var out := ""
var size := "terrarium"
var seed := "5"
var started := 0


func _initialize() -> void:
	started = Time.get_ticks_msec()
	out = OS.get_user_data_dir().path_join("picker-shots")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--out": out = args[i + 1]
			"--size": size = args[i + 1]
			"--seed": seed = args[i + 1]
	DirAccess.make_dir_recursive_absolute(out)
	run()


func run() -> void:
	var saves := out.path_join("saves")
	preload("res://ui/saves.gd").remove_tree(saves)
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.autoload = false
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	main.picker.select(size)
	main.picker._seed.text = seed
	var names := ["low", "medium", "high"]
	main.picker._water.set_value_no_signal(0)
	main.picker._water_text()
	main.picker.generate(4)
	for step in 3:
		if step > 0:
			main.picker._water.value = step
		if not await until(func(): return main.picker.ready_count() == 4, 240.0):
			print("PICKER SHOTS: terrains not drawn in time")
			return done()
		await seconds(6.0)  # the thumbnails' renderer draws its first frames
		await shot("picker-" + names[step])
	main.picker._water.value = 0
	await until(func(): return main.picker.ready_count() == 4, 240.0)
	await seconds(1.0)
	main.picker._cards[0]["pick"].pressed.emit()
	await until(func(): return str(main.session.founding().get("phase", "")) == "shower", 120.0)
	await seconds(1.0)
	await shot("founding-1")
	await until(_late_in_founding, 300.0)
	await shot("founding-2")
	await until(func(): return main.has_world(), 300.0)
	print("PICKER SHOTS founding: ", main.session.founding())
	await seconds(3.0)
	await shot("picker-founded")
	done()


func _late_in_founding() -> bool:
	var f: Dictionary = main.session.founding()
	return f["state"] != "founding" or str(f.get("phase", "")) in ["lake_rest", "stream_watch", "placing"]


func done() -> void:
	print("PICKER SHOTS done in %d s: %s" % [(Time.get_ticks_msec() - started) / 1000, out])
	preload("res://ui/saves.gd").remove_tree(out.path_join("saves"))
	DirAccess.remove_absolute(out.path_join("saves.cfg"))
	quit(0)


func shot(name: String) -> void:
	await RenderingServer.frame_post_draw
	var img := root.get_texture().get_image()
	img.save_png(out.path_join(name + ".png"))
	print("SHOT ", out.path_join(name + ".png"))


func frames(n: int) -> void:
	for i in n:
		await process_frame


func seconds(s: float) -> void:
	await create_timer(s).timeout


func until(cond: Callable, timeout_s: float) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while not cond.call():
		if Time.get_ticks_msec() > deadline:
			return false
		await process_frame
	return true
