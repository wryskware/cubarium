extends SceneTree
## Pictures of the tabbed page (Field guide / Populations / Settings) in the running app,
## driven without input. Needs a renderer, so it runs on a private display (never on
## Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/tabs_shots.gd -- --open SAVE --out DIR
## SAVE is a slot directory (e.g. from `cubarium-world found`). Writes DIR/tabs-*.png over
## the running world: the Field guide tab from G, the pause menu, the Settings tab from
## the menu's shortcut (Movement focused, then Stepped with Subpixel focused), the Field
## guide tab from the menu, the tab strip focused. With `--no-world` instead of `--open`:
## the pause menu and the Settings tab with no world (Field guide off). `--dots VARIANT`
## draws Gridlite's unlit dots another way (tests/dot_variants.gd).

var main
var out := ""
var started := 0


func _initialize() -> void:
	started = Time.get_ticks_msec()
	out = OS.get_user_data_dir().path_join("tabs-shots")
	var save := ""
	var no_world := false
	var args := OS.get_cmdline_user_args()
	for i in args.size():
		match args[i]:
			"--out": out = args[i + 1]
			"--open": save = args[i + 1]
			"--no-world": no_world = true
			"--dots": preload("res://tests/dot_variants.gd").apply_neon(args[i + 1])
	DirAccess.make_dir_recursive_absolute(out)
	if save == "" and not no_world:
		push_error("tabs_shots: needs --open SAVE or --no-world")
		quit(1)
		return
	run(save)


func run(save: String) -> void:
	var saves := out.path_join("saves")
	Saves.remove_tree(saves)
	DirAccess.make_dir_recursive_absolute(saves)
	if save != "":
		_copy(save, saves.path_join("Riverside"))
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	if save == "":
		main.autoload = false
	root.add_child(main)
	await frames(3)

	if save == "":
		# No world: the picker opens over the menu; back to the menu, then its Settings.
		main.close_screen(main.picker)
		await seconds(0.5)
		await shot("tabs-nw-1-pause-menu")
		main._on_menu("settings")
		await seconds(0.5)
		await shot("tabs-nw-2-settings")
		done()
		return

	await until(func(): return main.has_world() and main.busy() == "", 120.0)
	await seconds(3.0)

	# G over the running world: the Field guide tab.
	main.open_guide()
	await seconds(1.5)
	await shot("tabs-1-guide")
	main.close_tabbed()

	main.escape()
	await seconds(0.5)
	await shot("tabs-2-pause-menu")

	main._on_menu("settings")
	await seconds(0.8)
	await shot("tabs-3-settings")
	main.settings_page.choose_movement("stepped")
	main.settings_page._subpixel.grab_focus()
	await seconds(0.5)
	await shot("tabs-4-settings-stepped")
	main.settings_page.choose_movement("smooth")
	main.tabbed.buttons["settings"].grab_focus()
	await seconds(0.3)
	await shot("tabs-5-settings-tab-focused")
	main.close_tabbed()

	main._on_menu("guide")
	await seconds(1.5)
	await shot("tabs-6-guide-from-menu")
	done()


const Saves := preload("res://ui/saves.gd")


func done() -> void:
	print("TABS SHOTS done in %d s: %s" % [(Time.get_ticks_msec() - started) / 1000, out])
	main.queue_free()
	await frames(2)
	Saves.remove_tree(out.path_join("saves"))
	Saves.remove_tree(out.path_join("saves.staging"))
	DirAccess.remove_absolute(out.path_join("saves.cfg"))
	quit(0)


func shot(name: String) -> void:
	await frames(3)
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


func _copy(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))
