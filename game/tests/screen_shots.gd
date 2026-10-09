extends SceneTree
## Pictures of the neon New world picker and Settings page in the running app, driven
## without input, for comparing with round 4 (design/ui/round4/index.html). Needs a
## renderer, so it runs on a private display (never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/screen_shots.gd -- --out DIR [--size terrarium] [--seed 5]
##       [--budget 230]
## Writes DIR/picker-drawing.png (cards drawing, the first focused), picker-ready.png,
## picker-founding.png, then settings-subpixel.png (Subpixel focused) and
## settings-stepped.png (Stepped, Subpixel greyed) over the founded world, or over no world
## if the founding hasn't finished within --budget seconds of the start.

var main
var out := ""
var size := "terrarium"
var seed := "5"
var budget_s := 230.0
var started := 0


func _initialize() -> void:
	started = Time.get_ticks_msec()
	out = OS.get_user_data_dir().path_join("screen-shots")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--out": out = args[i + 1]
			"--size": size = args[i + 1]
			"--seed": seed = args[i + 1]
			"--budget": budget_s = float(args[i + 1])
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
	main.open_picker()
	await frames(1)
	main.picker.select(size)
	main.picker._seed.text = seed
	main.picker.generate(4)
	# Still drawing (terrains take a few seconds), the first card focused.
	await seconds(0.5)
	main.picker._cards[0]["root"].grab_focus()
	await shot("picker-drawing")
	await until(func(): return main.picker.ready_count() == 4, 120.0)
	await seconds(4.0)
	main.picker._cards[1]["root"].grab_focus()
	await shot("picker-ready")
	main.picker._cards[1]["root"].pressed.emit()
	await until(func(): return main.session.founding().get("progress", 0.0) > 0.3 or main.session.founding().get("state", "") != "founding", 120.0)
	await shot("picker-founding")
	var left := budget_s - (Time.get_ticks_msec() - started) / 1000.0
	await until(func(): return main.has_world(), maxf(left, 1.0))
	print("SCREEN SHOTS world running: ", main.has_world())
	await seconds(2.0)
	main.open_settings()
	await frames(2)
	main.settings_page.set_values("smooth", true)
	main.settings_page._subpixel.grab_focus()
	await shot("settings-subpixel")
	main.settings_page.focus_first()
	main.settings_page.choose_movement("stepped")
	await frames(2)
	await shot("settings-stepped")
	print("SCREEN SHOTS done in %d s: %s" % [(Time.get_ticks_msec() - started) / 1000, out])
	main.picker.leave()
	preload("res://ui/saves.gd").remove_tree(saves)
	DirAccess.remove_absolute(saves + ".cfg")
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
