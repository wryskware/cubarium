extends SceneTree
## Fast shell/storage mechanism checks, with no terrain generation or world ticks.

const MAIN := preload("res://main.tscn")
const Saves := preload("res://ui/saves.gd")
var failures := 0
var quits := 0

class FakeSession extends RefCounted:
	var live := false
	var home_path := ""
	var generation := 0
	var root_path := ""
	var rows: Array = []
	var notices: Array = []
	var save_notices: Array = []
	var fail_autosave := false
	## Requested saves stay in flight (their notices held) until `release`.
	var hold := false
	var held: Array = []
	var autosaves := 0
	## What an autosave writes into the slot's `state` file: the world as it is now.
	var world_state := "running world"
	var loaded_path := ""
	var loaded_state := ""

	func status() -> Dictionary:
		return {"running": live, "paused": true, "saving": false, "loading": false, "generation": generation}
	func founding() -> Dictionary:
		return {"state": "done"}
	func home() -> String:
		return home_path if live else ""
	func attach_home(path: String) -> void:
		home_path = path
	func pause() -> void:
		pass
	func resume() -> void:
		pass
	func cancel() -> void:
		pass
	func close() -> void:
		live = false
	func query(kind: String, _args):
		return rows if kind == "saves" else {}
	func take_notices() -> Array:
		var result := notices
		notices = []
		return result
	func take_save_notices() -> Array:
		var result := save_notices
		save_notices = []
		return result
	func promote_into(_index: int, path: String) -> String:
		home_path = path
		live = true
		generation += 1
		return ""
	func save(path: String) -> void:
		DirAccess.make_dir_recursive_absolute(path)
		var file := FileAccess.open(path.path_join("fixture"), FileAccess.WRITE)
		file.store_string("snapshot")
		file.close()
		notices.append({"kind": "saved", "path": path})
	func autosave() -> String:
		autosaves += 1
		var notice := {}
		if fail_autosave:
			notice = {"kind": "autosave_failed", "text": "fixture disk failure", "requested": true}
		else:
			DirAccess.make_dir_recursive_absolute(home_path)
			var file := FileAccess.open(home_path.path_join("state"), FileAccess.WRITE)
			file.store_string(world_state)
			file.close()
			notice = {"kind": "autosaved", "path": home_path, "requested": true}
		(held if hold else save_notices).append(notice)
		return ""
	func release() -> void:
		save_notices.append_array(held)
		held = []
	func load_with_home(path: String, attached: String) -> void:
		loaded_path = path
		loaded_state = FileAccess.get_file_as_string(path.path_join("state"))
		home_path = attached
		live = true
		generation += 1
		notices.append({"kind": "loaded"})

class ThumbnailView extends Node:
	var frame_generation := 100
	var lighting_ready := false
	var captures := 0
	func capture(_size: Vector2i) -> Image:
		captures += 1
		var image := Image.create_empty(2, 2, false, Image.FORMAT_RGBA8)
		image.fill(Color.MAGENTA)
		return image

func _initialize() -> void:
	run.call_deferred()

func frames(n: int) -> void:
	for i in n:
		await process_frame

func check(ok: bool, message: String) -> void:
	if not ok:
		failures += 1
		push_error(message)

func run() -> void:
	var temp := "/tmp/cubarium-startup-save-%d" % Time.get_ticks_usec()
	var main = MAIN.instantiate()
	main.saves_root = temp.path_join("saves")
	main.settings_path = temp.path_join("shell.cfg")
	main.autoload = false
	main.quit_hook = func(): quits += 1
	root.add_child(main)
	await process_frame
	check(main.menu.visible and main.menu.startup and not main.picker.visible, "startup stays on title menu")
	check(main.menu.buttons["resume"].disabled, "Continue is disabled without a save")
	check(not main.menu.buttons["save"].visible and main.menu.buttons["load"].visible, "title offers startup actions")
	var picker = main.picker
	var default_spec: Dictionary = picker.spec()
	var chosen: Dictionary = picker.sizes[picker._size.index]
	check(default_spec["size"] == chosen["size"].map(func(v): return int(v)), "the default size sends its catalog dimensions")
	picker.select("custom")
	for i in 3:
		picker._dimension_inputs[i].text = str([192, 96, 24][i])
	check(picker.dimensions_problem() == "", "custom dimensions accept positive integers")
	check(picker.spec()["size"] == [192, 96, 24], "custom spec sends chosen dimensions")
	for bad in ["0", "width"]:
		picker._dimension_inputs[0].text = bad
		check(picker.generate(1) != "", "invalid dimension is rejected before generation: " + bad)
	# Whatever the budget is, a world a thousand times larger on every side is over it.
	for i in 3:
		picker._dimension_inputs[i].text = str([192, 96, 24][i] * 1000)
	check(picker.generate(1) != "", "over-budget dimensions are rejected before generation")
	check(main.session.workers() == 0, "invalid custom dimensions start no generation workers")
	picker.select("widescreen")
	var fake := FakeSession.new()
	main.session = fake
	check(main.found_candidate(0) == "", "founding assigns a home")
	var first := fake.home_path.get_file()
	picker._founding = true
	picker._found_generation = fake.generation
	picker._follow_founding()
	check(picker._founding and main.pending_autosave == "" and not main.saves.exists(first), "worker done waits for installation before initial autosave")
	fake.generation += 1
	picker._follow_founding()
	check(not picker._founding and main.pending_autosave == "initial", "installation starts the initial autosave")
	await process_frame
	await process_frame
	check(main.saves.exists(first), "founding creates the initial autosave")
	check(main.saves.new_home() != first, "another world gets an independent autosave home")
	var live_view: Node = main.world_view
	var thumbnail_view := ThumbnailView.new()
	main.add_child(thumbnail_view)
	main.world_view = thumbnail_view
	check(main._current_thumbnail() == null and thumbnail_view.captures == 0, "thumbnail waits for lit frame")
	thumbnail_view.lighting_ready = true
	main.thumbnail_handoff = {"session": fake.generation - 1, "frame": thumbnail_view.frame_generation}
	check(main._current_thumbnail() == null, "thumbnail rejects previous world's frame during handoff")
	thumbnail_view.frame_generation += 1
	check(main._current_thumbnail() != null, "new hosted frame can differ from the session generation counter")
	main.pending_thumbnail = {"path": fake.home_path, "generation": fake.generation}
	main._refresh_autosave_thumbnail()
	check(FileAccess.file_exists(fake.home_path.path_join(Saves.THUMB)), "current frame supplies the completed autosave thumbnail")
	main.pending_thumbnail = {"path": fake.home_path, "generation": fake.generation - 1}
	var captures := thumbnail_view.captures
	main._refresh_autosave_thumbnail()
	check(main.pending_thumbnail.is_empty() and thumbnail_view.captures == captures, "stale thumbnail cannot attach to a replacement world")
	main.world_view = live_view
	thumbnail_view.free()
	check(main.save_slot("Snapshot") == "", "manual snapshot is accepted")
	await process_frame
	await process_frame
	check(fake.home_path.get_file() == first, "manual snapshot preserves attached home")
	check(main.saves.home_of("Snapshot") == first, "manual snapshot stores its attached home")
	fake.rows = [{"name": "Snapshot", "loadable": true}, {"name": first, "loadable": true}]
	check(main._last_slot() == first, "Continue selects attached autosave ahead of snapshot")
	var renamed: String = main.saves.rename_target(first, "Named world")
	check(main.saves.rename(first, renamed) == "", "autosave directory can be renamed")
	main.renamed_slot(first, renamed)
	check(fake.home_path.get_file() == renamed and main.saves.home_of("Snapshot") == renamed, "rename updates live home and snapshot link")
	check(main.saves.rename("Snapshot", "Other snapshot") == "", "snapshot can be renamed independently")
	main.renamed_slot("Snapshot", "Other snapshot")
	check(fake.home_path.get_file() == renamed, "snapshot rename preserves attached home")
	var autosaves := fake.autosaves
	main.load_slot("Other snapshot")
	await frames(3)
	check(fake.home_path.get_file() == renamed, "loading a snapshot reattaches the world's home")
	check(fake.autosaves == autosaves + 1, "loading a snapshot saves the running world first")

	# (a) The running world's own autosave is its restore point: loading it loads it as it is.
	var own: String = fake.home_path.get_file()
	var own_state: String = main.saves.path_of(own).path_join("state")
	var file := FileAccess.open(own_state, FileAccess.WRITE)
	file.store_string("restore point")
	file.close()
	fake.world_state = "the world as it is now"
	main.load_slot(own)
	await frames(3)
	check(fake.loaded_path == main.saves.path_of(own) and fake.loaded_state == "restore point", "(a) loading the running world's own autosave loads it unchanged")
	check(FileAccess.get_file_as_string(own_state) == "restore point", "(a) the restore point is not overwritten by the load")

	# (c) A periodic save failing while a load waits for its own save keeps the load queued.
	fake.hold = true
	fake.loaded_path = ""
	main.load_slot("Other snapshot")
	await frames(1)
	fake.save_notices.append({"kind": "autosave_failed", "text": "periodic fixture failure", "requested": false})
	await frames(1)
	check(main.queued_load == "Other snapshot" and fake.loaded_path == "", "(c) a failed periodic save keeps the queued load")
	fake.release()
	await frames(3)
	check(fake.loaded_path == main.saves.path_of("Other snapshot"), "(c) the requested save completing then performs the load")
	# ... and the same for Quit.
	var before := quits
	main.request_quit()
	await frames(1)
	fake.save_notices.append({"kind": "autosave_failed", "text": "periodic fixture failure", "requested": false})
	await frames(1)
	check(quits == before and main.quit_after_save, "(c) a failed periodic save keeps the queued quit")
	fake.release()
	await frames(3)
	check(quits == before + 1, "(c) the requested save completing then quits")

	# (d) Quit pressed while a switch save is finishing ends in a quit.
	before = quits
	main._request_autosave("switch")
	main.request_quit()
	await frames(1)
	check(quits == before and main.quit_after_save, "(d) quit waits for the switch save")
	fake.hold = false
	fake.release()
	await frames(3)
	check(quits == before + 1, "(d) quit pressed during a switch save ends in a quit")

	main.open_menu()
	check(not main.menu.startup and main.menu.buttons["resume"].get_node("Content/Label").text == " Resume ", "running-world menu retains Resume")

	# (b) A failed quit save keeps the app open and offers to quit without saving.
	before = quits
	fake.fail_autosave = true
	main.request_quit()
	await frames(3)
	check(quits == before and not main.quit_after_save and main.dialog.is_open(), "(b) a failed quit save asks before quitting unsaved")
	main.dialog._cancel.pressed.emit()
	check(quits == before and main.menu.visible, "(b) declining keeps the app open on the menu")
	main.request_quit()
	await frames(3)
	main.dialog._ok.pressed.emit()
	check(quits == before + 1, "(b) after a failed quit save the player can still quit")
	fake.fail_autosave = false
	before = quits
	main.request_quit()
	await frames(3)
	check(quits == before + 1, "Quit waits for a completed autosave")

	# A typed name may use the whole limit, even for an autosave whose suffix is added.
	var long_auto := "World 7" + Saves.AUTOSAVE_SUFFIX
	DirAccess.make_dir_recursive_absolute(main.saves.path_of(long_auto))
	var full := "x".repeat(Saves.MAX_NAME)
	check(main.saves.rename(long_auto, main.saves.rename_target(long_auto, full)) == "", "an autosave can be renamed to a name of the full length")
	check(main.saves.rename(full + Saves.AUTOSAVE_SUFFIX, main.saves.rename_target(full + Saves.AUTOSAVE_SUFFIX, full + "x")) != "", "a typed name over the limit is refused")
	check(main.saves.delete(renamed) == "", "active autosave can be deleted")
	main.deleted_slot(renamed)
	check(not fake.live and main.menu.startup, "deleting active autosave closes world to title")
	check(main.saves.home_of("Other snapshot") == renamed, "retained snapshots keep their world identity after autosave deletion")
	var reserved := "World 1" + Saves.AUTOSAVE_SUFFIX
	var retained: String = main.saves.path_of("Retained snapshot")
	DirAccess.make_dir_recursive_absolute(retained)
	check(main.saves.write_home(retained, reserved) == "", "retained snapshot stores a home reference")
	check(main.saves.new_home() != reserved, "a retained snapshot reserves its deleted world's name")
	main.queue_free()
	await process_frame

	# (e) A crash between a replacement's two renames leaves only `.previous-<name>`:
	# start-up puts it back when the slot is missing, and leaves a completed slot alone.
	var crash_root := temp.path_join("crash-saves")
	var crash_staging := crash_root + ".staging"
	for pair in [["World 9" + Saves.AUTOSAVE_SUFFIX, ".previous-World 9" + Saves.AUTOSAVE_SUFFIX, "previous world"],
			["World 8", ".previous-World 8", "older world"]]:
		DirAccess.make_dir_recursive_absolute(crash_staging.path_join(pair[1]))
		var f := FileAccess.open(crash_staging.path_join(pair[1]).path_join("completed"), FileAccess.WRITE)
		f.store_string(pair[2])
		f.close()
	DirAccess.make_dir_recursive_absolute(crash_root.path_join("World 8"))
	var newer := FileAccess.open(crash_root.path_join("World 8").path_join("completed"), FileAccess.WRITE)
	newer.store_string("newer world")
	newer.close()
	Saves.new(crash_root)
	var orphan := "World 9" + Saves.AUTOSAVE_SUFFIX
	check(FileAccess.get_file_as_string(crash_root.path_join(orphan).path_join("completed")) == "previous world", "(e) an orphaned previous save is restored at start-up")
	check(not DirAccess.dir_exists_absolute(crash_staging.path_join(".previous-" + orphan)), "(e) the restored save leaves the staging directory")
	check(FileAccess.get_file_as_string(crash_root.path_join("World 8").path_join("completed")) == "newer world", "(e) a completed slot is never replaced by its previous save")
	Saves.remove_tree(crash_staging)
	Saves.remove_tree(temp)
	Saves.remove_tree(temp.path_join("saves") + ".staging")
	print("STARTUP SAVE TEST: %d failures" % failures)
	quit(0 if failures == 0 else 1)
