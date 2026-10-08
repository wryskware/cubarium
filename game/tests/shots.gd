extends SceneTree
## Pictures of every shell screen, driven without input. Needs a renderer, so it runs on a
## private display (never on Wrysk's screen):
##   scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/shots.gd -- --out DIR
## Writes DIR/01-founding.png … 08-rename.png from the root viewport and quits.

var main
var out := ""
var started := 0


func _initialize() -> void:
	started = Time.get_ticks_msec()
	out = OS.get_user_data_dir().path_join("shots")
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		if args[i] == "--out":
			out = args[i + 1]
	DirAccess.make_dir_recursive_absolute(out)
	run()


func run() -> void:
	var saves := out.path_join("saves")
	preload("res://ui/saves.gd").remove_tree(saves)
	var args := OS.get_cmdline_user_args()
	var at := args.find("--open")
	if at >= 0 and at + 1 < args.size():
		await run_open(saves, args[at + 1])
		return
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.autoload = false
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	main.picker.select("terrarium")
	main.picker._seed.text = "11"
	main.picker.generate()
	await seconds(4.0)
	await shot("01-founding")
	if not await until(func(): return main.picker.ready_count() == main.picker._cards.size(), 240.0):
		print("SHOTS: candidates did not found in time")
		return done()
	await frames(4)
	await shot("02-new-world")

	main.picker.open_large(1)
	await frames(2)
	var big = main.picker.large_view()
	if big != null and big.has_method("pan_by"):
		big.pan_by(Vector2(0.35, 0.0))  # panned a third of the way round the ring
	elif big != null:
		big._zoom = 2.0
		big._centre = Vector2(big._dims.x * 0.3, big._dims.y * 0.6)
	await seconds(1.5)
	await shot("03-candidate")
	main.picker.close_large()

	main.picker._cards[0]["pick"].pressed.emit()
	await until(func(): return main.has_world(), 20.0)
	await seconds(2.0)
	await shot("04-live")

	var animals = main.session.query("animals", null)
	if animals is Array and animals.size() > 1:
		var id: int = animals[animals.size() / 2]["id"]
		var wv = main.world_view
		if wv.has_method("_to_screen"):
			Input.warp_mouse(wv._to_screen(wv._animal_px(id)))
		elif wv.get("camera") != null and wv.organisms.animal_at(id) != null:
			Input.warp_mouse(wv.camera.unproject_position(wv.organisms.animal_at(id)))
		main.world_view.animal_clicked.emit(id)
		await seconds(0.3)
		main.world_view.animal_hovered.emit(id)
		await seconds(1.5)
		await shot("05-inspect")
		main.world_view.animal_hovered.emit(-1)
		main.inspect.close()

	main.escape()
	await seconds(0.5)
	await shot("06-pause-menu")

	main._on_menu("save")
	for name in ["Riverside", "Second look"]:
		main.save_slot(name)
		await until(func(): return main.busy() == "", 30.0)
		await seconds(1.0)
	_older_copy(main.saves.path_of("Riverside"), main.saves.path_of("From last week"))
	main.slots.open("load")
	await seconds(0.5)
	await shot("07-save-slots")
	main.slots._rename("Riverside")
	await seconds(0.3)
	await shot("08-rename")
	main.dialog.cancel()
	done()


func done() -> void:
	print("SHOTS done in %d s: %s" % [(Time.get_ticks_msec() - started) / 1000, out])
	preload("res://ui/saves.gd").remove_tree(out.path_join("saves"))
	preload("res://ui/saves.gd").remove_tree(out.path_join("saves.staging"))
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
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))
	var meta := to.path_join("meta.json")
	var text := FileAccess.get_file_as_string(meta)
	var m := RegEx.create_from_string("\"world\":\\s*(\\d+)").search(text)
	if m != null:
		text = text.substr(0, m.get_start()) + "\"world\": %d" % (int(m.get_string(1)) - 1) + text.substr(m.get_end())
	var f := FileAccess.open(meta, FileAccess.WRITE)
	f.store_string(text)
	f.close()


## `--open SAVE`: copy a ready-made save in as a slot and let start-up open it (the wide
## world founds for minutes); then the live world, the inspect panel, a save of it with
## its thumbnail, and the slots.
func run_open(saves: String, save: String) -> void:
	_copy(save, saves.path_join("Wide world"))
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): pass
	root.add_child(main)
	await frames(3)
	await until(func(): return main.has_world() and main.busy() == "", 120.0)
	await seconds(3.0)
	await shot("09-open-live")
	var animals = main.session.query("animals", null)
	if animals is Array and animals.size() > 0:
		var id: int = animals[0]["id"]
		main.world_view.animal_clicked.emit(id)
		await seconds(2.5)
		await shot("10-open-inspect")
		main.inspect.close()
	main.escape()
	main._on_menu("save")
	main.save_slot("Wide world")
	await until(func(): return main.busy() == "", 60.0)
	main.slots.refresh()
	await seconds(0.5)
	await shot("11-open-slots")
	done()


func _copy(from: String, to: String) -> void:
	DirAccess.make_dir_recursive_absolute(to)
	var d := DirAccess.open(from)
	for f in d.get_files():
		DirAccess.copy_absolute(from.path_join(f), to.path_join(f))
