extends SceneTree
## A picture of the dialog a failed quit save opens, over a running world. Needs a
## renderer, so it runs on a private display (never on Wrysk's screen):
##   CUBARIUM_GODOT_GPU=1 scripts/hidden.sh godot --path game --resolution 1920x1080 \
##       --script res://tests/quit_unsaved_shot.gd -- --open SAVE --out DIR
## SAVE is a slot directory. The copied saves root is made read-only, so the quit save
## really fails; it is made writable again before the copy is removed.

const Saves := preload("res://ui/saves.gd")

var main
var out := ""


func _initialize() -> void:
	out = OS.get_user_data_dir().path_join("quit-unsaved-shot")
	var save := ""
	var args := OS.get_cmdline_user_args()
	for i in args.size() - 1:
		match args[i]:
			"--out": out = args[i + 1]
			"--open": save = args[i + 1]
	DirAccess.make_dir_recursive_absolute(out)
	if save == "":
		push_error("quit_unsaved_shot: needs --open SAVE")
		quit(1)
		return
	run(save)


func run(save: String) -> void:
	var saves := out.path_join("saves")
	Saves.remove_tree(saves)
	DirAccess.make_dir_recursive_absolute(saves.path_join(save.get_file()))
	var d := DirAccess.open(save)
	for f in d.get_files():
		DirAccess.copy_absolute(save.path_join(f), saves.path_join(save.get_file()).path_join(f))
	main = load("res://main.tscn").instantiate()
	main.saves_root = saves
	main.settings_path = saves + ".cfg"
	main.quit_hook = func(): print("QUIT HOOK")
	root.add_child(main)
	await frames(3)
	main._on_menu("resume")
	if not await until(func(): return main.has_world() and main.busy() == "", 180.0):
		push_error("the world did not open")
	await seconds(4.0)
	OS.execute("chmod", ["a-w", saves])
	main.request_quit()
	var asked: bool = await until(func(): return main.dialog.is_open(), 60.0)
	await seconds(0.5)
	if asked:
		await shot("quit-unsaved-dialog")
	else:
		push_error("no dialog after the failed quit save")
	OS.execute("chmod", ["u+w", saves])
	main.queue_free()
	await frames(2)
	Saves.remove_tree(saves)
	Saves.remove_tree(saves + ".staging")
	DirAccess.remove_absolute(saves + ".cfg")
	quit(0 if asked else 1)


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
