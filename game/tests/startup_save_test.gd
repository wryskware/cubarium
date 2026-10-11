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
		if fail_autosave:
			save_notices.append({"kind": "autosave_failed", "text": "fixture disk failure", "requested": true})
		else:
			DirAccess.make_dir_recursive_absolute(home_path)
			save_notices.append({"kind": "autosaved", "path": home_path, "requested": true})
		return ""
	func load_with_home(_path: String, attached: String) -> void:
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
	var default_dims: Array = default_spec["size"]
	var aspect := float(default_dims[0]) / (float(default_dims[1]) + float(default_dims[2]) * sin(deg_to_rad(30.0)))
	check(absf(aspect - 16.0 / 9.0) < 0.02, "default dimensions project to approximately 16:9 at 30 degrees")
	picker.select("custom")
	for i in 3:
		picker._dimension_inputs[i].text = str([192, 96, 24][i])
	check(picker.dimensions_problem() == "", "custom dimensions accept positive integers")
	check(picker.spec()["size"] == [192, 96, 24], "custom spec sends chosen dimensions")
	for bad in ["0", "width"]:
		picker._dimension_inputs[0].text = bad
		check(picker.generate(1) != "", "invalid dimension is rejected before generation: " + bad)
	for i in 3:
		picker._dimension_inputs[i].text = str([1024, 256, 128][i])
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
	main.load_slot("Other snapshot")
	await process_frame
	await process_frame
	await process_frame
	check(fake.home_path.get_file() == renamed, "loading a snapshot reattaches the world's home")
	main.open_menu()
	check(not main.menu.startup and main.menu.buttons["resume"].get_node("Content/Label").text == " Resume ", "running-world menu retains Resume")
	fake.fail_autosave = true
	main.request_quit()
	await process_frame
	await process_frame
	await process_frame
	check(quits == 0 and not main.quit_after_save, "a failed quit save keeps the app open")
	fake.fail_autosave = false
	main.request_quit()
	await process_frame
	await process_frame
	await process_frame
	check(quits == 1, "Quit waits for a completed autosave")
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
	Saves.remove_tree(temp)
	Saves.remove_tree(temp.path_join("saves") + ".staging")
	print("STARTUP SAVE TEST: %d failures" % failures)
	quit(0 if failures == 0 else 1)
