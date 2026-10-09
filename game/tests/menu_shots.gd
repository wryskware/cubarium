extends SceneTree
## Pictures of the neon menus over a running world, driven without input. Needs a
## renderer, so it runs on a private display (never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/menu_shots.gd -- --open SAVE --out DIR
## SAVE is a slot directory (e.g. from `cubarium-world found`). Writes DIR/menu-*.png: the
## paused badge, the inspect panel on a live animal, the pause menu, the save and load
## slots (three slots, one from another version, one focused), the dialog as a
## confirmation and as the name prompt with a problem, the notices, the busy panel while
## a slot opens, and the field guide with Animals collapsed.

var main
var out := ""
var started := 0


func _initialize() -> void:
	started = Time.get_ticks_msec()
	out = OS.get_user_data_dir().path_join("menu-shots")
	var save := ""
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--out": out = args[i + 1]
			"--open": save = args[i + 1]
	DirAccess.make_dir_recursive_absolute(out)
	if save == "":
		push_error("menu_shots: needs --open SAVE")
		quit(1)
		return
	run(save)


func run(save: String) -> void:
	var saves := out.path_join("saves")
	Saves.remove_tree(saves)
	_copy(save, saves.path_join("Riverside"))
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	await until(func(): return main.has_world() and main.busy() == "", 120.0)
	await seconds(3.0)

	main.set_player_paused(true)
	await seconds(0.5)
	await shot("menu-01-paused-badge")
	main.set_player_paused(false)

	var animals = main.session.query("animals", null)
	if animals is Array and animals.size() > 0:
		var id: int = animals[animals.size() / 2]["id"]
		main.world_view.animal_clicked.emit(id)
		await seconds(2.5)
		await shot("menu-02-inspect")
		main.inspect.close()

	main.escape()
	await seconds(0.5)
	await shot("menu-03-pause-menu")

	main._on_menu("save")
	main.save_slot("Second look")
	await until(func(): return main.busy() == "", 60.0)
	await seconds(0.5)
	_older_copy(main.saves.path_of("Riverside"), main.saves.path_of("From last week"))
	main.slots.refresh()
	await frames(2)
	var rows: Dictionary = main.slots._rows
	if rows.has("Riverside"):
		rows["Riverside"]["overwrite"].grab_focus()
	await seconds(0.5)
	await shot("menu-04-save-slots")
	main.slots.open("load")
	await seconds(0.5)
	await shot("menu-05-load-slots")

	main.slots._overwrite("Riverside")
	await seconds(0.3)
	await shot("menu-06-dialog-confirm")
	main.dialog.cancel()
	main.slots._rename("Riverside")
	main.dialog._field.text = "Second look"
	main.dialog._confirm()
	await seconds(0.3)
	await shot("menu-07-dialog-name")
	main.dialog.cancel()

	main.close_screen(main.slots)
	main.escape()
	await frames(2)
	main.notices.add("Couldn't load \"From last week\": this save is from another version of Cubarium.", true)
	await seconds(0.6)
	await shot("menu-08-notices")

	main.load_slot("Riverside")
	await frames(3)
	await shot("menu-09-busy")
	await until(func(): return main.busy() == "", 60.0)
	await seconds(2.0)

	for n in main.notices.get_children():
		n.queue_free()
	main._on_menu("guide")
	await seconds(1.5)
	main.guide._set_animals_open(false)
	await frames(2)
	main.guide._animals_toggle.grab_focus()
	await seconds(0.5)
	await shot("menu-10-guide-animals-collapsed")
	main.guide._set_plants_open(true)
	await seconds(0.3)
	await shot("menu-11-guide-animals-collapsed-plants-open")
	main.guide._set_animals_open(true)
	main.guide._set_plants_open(false)
	done()


const Saves := preload("res://ui/saves.gd")


func done() -> void:
	print("MENU SHOTS done in %d s: %s" % [(Time.get_ticks_msec() - started) / 1000, out])
	main.queue_free()
	await frames(2)
	Saves.remove_tree(out.path_join("saves"))
	Saves.remove_tree(out.path_join("saves.staging"))
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


## A copy of a slot whose meta.json names a world schema from another version.
func _older_copy(from: String, to: String) -> void:
	_copy(from, to)
	var meta := to.path_join("meta.json")
	var text := FileAccess.get_file_as_string(meta)
	var m := RegEx.create_from_string("\"world\":\\s*(\\d+)").search(text)
	if m != null:
		text = text.substr(0, m.get_start()) + "\"world\": %d" % (int(m.get_string(1)) - 1) + text.substr(m.get_end())
	var f := FileAccess.open(meta, FileAccess.WRITE)
	f.store_string(text)
	f.close()


func _copy(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))
