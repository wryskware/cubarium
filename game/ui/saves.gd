extends RefCounted
## **Save slots on disk**: one named directory per slot under `root` (`user://saves/`,
## globalized), each holding the session's save (`world-N.voxel`, its layers and
## `meta.json`) and the shell's `thumb.png`.
##
## A save is written into a staging directory beside the root and moved into place only
## once the session says `saved`: overwriting a slot never mixes two worlds' files (a
## slot's load takes its newest `world-N.voxel` by tick, so an older world's later tick
## would win), and a failed save leaves the old slot as it was.

const THUMB := "thumb.png"
const MAX_NAME := 40
const AUTOSAVE_SUFFIX := " (autosave)"
const HOME := "world-home"

var root := ""
var staging := ""


func _init(root_dir: String) -> void:
	root = root_dir.rstrip("/")
	staging = root + ".staging"
	DirAccess.make_dir_recursive_absolute(root)
	recover()


## Put back any slot a crash left half replaced. A replacement (here, or the extension's
## autosave in world_save.rs) moves the slot to `<root>.staging/.previous-<name>` and then
## moves the new save in; a crash between the two leaves only the previous save, which is
## moved back when its slot is missing. Returns the names restored.
func recover() -> Array:
	var restored := []
	var d := DirAccess.open(staging)
	if d == null:
		return restored
	d.include_hidden = true
	for dir in d.get_directories():
		if not dir.begins_with(".previous-"):
			continue
		var slot := dir.trim_prefix(".previous-")
		if name_problem(slot) != "" or exists(slot):
			continue
		if DirAccess.rename_absolute(staging.path_join(dir), path_of(slot)) == OK:
			restored.append(slot)
		else:
			push_warning("Could not restore the previous save of \"%s\" from %s." % [slot, staging.path_join(dir)])
	return restored


## The slots, newest first, as the session's `saves` query lists them (read without
## loading any world), each with `thumb` (a path, or "") added.
func list(session) -> Array:
	var slots = session.query("saves", {"dir": root})
	if typeof(slots) != TYPE_ARRAY:
		return []
	for s in slots:
		var thumb := root.path_join(s["name"]).path_join(THUMB)
		s["thumb"] = thumb if FileAccess.file_exists(thumb) else ""
	return slots


func path_of(slot: String) -> String:
	return root.path_join(slot)


func exists(slot: String) -> bool:
	return DirAccess.dir_exists_absolute(path_of(slot))


## Why `slot` can't be a slot name, or "". An autosave's own suffix doesn't count toward
## the length: the limit is on what the player types.
func name_problem(slot: String) -> String:
	if slot.strip_edges() != slot or slot.is_empty():
		return "A name can't be empty or start or end with a space."
	if slot.trim_suffix(AUTOSAVE_SUFFIX).length() > MAX_NAME:
		return "A name is at most %d characters." % MAX_NAME
	if slot.begins_with(".") or not slot.is_valid_filename():
		return "A name can't start with a dot or hold any of  / \\ : * ? \" < > |"
	return ""


## Automatic snapshots reserve their suffix; they belong to worlds, not Save as.
func is_autosave(slot: String) -> bool:
	return slot.ends_with(AUTOSAVE_SUFFIX) or slot == "Autosave"


func new_home() -> String:
	# A retained snapshot reserves its world identity even after its autosave is deleted.
	var referenced := {}
	var d := DirAccess.open(root)
	if d != null:
		for name in d.get_directories():
			var link := path_of(name).path_join(HOME)
			if FileAccess.file_exists(link):
				referenced[FileAccess.get_file_as_string(link).strip_edges()] = true
	var n := 1
	while exists("World %d" % n) or exists("World %d%s" % [n, AUTOSAVE_SUFFIX]) or referenced.has("World %d%s" % [n, AUTOSAVE_SUFFIX]):
		n += 1
	return "World %d%s" % [n, AUTOSAVE_SUFFIX]


## An app snapshot carries its attached autosave name in a small shell-owned sidecar.
## Older saves sensibly keep the existing name/auto-name pairing.
func home_of(slot: String) -> String:
	if is_autosave(slot):
		return slot
	var file := path_of(slot).path_join(HOME)
	if FileAccess.file_exists(file):
		var home := FileAccess.get_file_as_string(file).strip_edges()
		if name_problem(home) == "" and is_autosave(home):
			return home
	return slot + AUTOSAVE_SUFFIX


func write_home(dir: String, home: String) -> String:
	var file := FileAccess.open(dir.path_join(HOME), FileAccess.WRITE)
	if file == null:
		return "Could not attach the world's autosave (%s)." % error_string(FileAccess.get_open_error())
	file.store_string(home)
	return "" if file.get_error() == OK else "Could not write the world's autosave link."


## The first free "Snapshot N".
func next_name() -> String:
	var n := 1
	while exists("Snapshot %d" % n):
		n += 1
	return "Snapshot %d" % n


## A clean staging directory for `slot`, to hand the session's `save`.
func begin(slot: String) -> String:
	var dir := staging.path_join(slot)
	remove_tree(dir)
	DirAccess.make_dir_recursive_absolute(dir)
	return dir


## Move a finished save from staging into its slot, replacing what was there, with its
## thumbnail. "" or why not.
func commit(slot: String, thumb: Image, home := "") -> String:
	var from := staging.path_join(slot)
	if not DirAccess.dir_exists_absolute(from):
		return "the save went missing before it could be put in its slot"
	if thumb != null and not thumb.is_empty():
		var png_error := thumb.save_png(from.path_join(THUMB))
		if png_error != OK:
			return "could not save the thumbnail (%s)" % error_string(png_error)
	if home != "":
		var problem := write_home(from, home)
		if problem != "":
			return problem
	var to := path_of(slot)
	var backup := staging.path_join(".previous-" + slot)
	remove_tree(backup)
	var had_slot := exists(slot)
	if had_slot:
		var keep_error := DirAccess.rename_absolute(to, backup)
		if keep_error != OK:
			return "could not keep the previous save (%s)" % error_string(keep_error)
	var err := DirAccess.rename_absolute(from, to)
	if err != OK:
		if had_slot:
			var restore_error := DirAccess.rename_absolute(backup, to)
			if restore_error != OK:
				return "could not move or restore the save; the previous save is at %s" % backup
		return "could not move the save into its slot (%s)" % error_string(err)
	remove_tree(backup)
	return ""


func rename(slot: String, to: String) -> String:
	if not is_autosave(slot) and is_autosave(to):
		return "This name belongs to a world autosave. Choose a snapshot name."
	to = rename_target(slot, to)
	var problem := name_problem(to)
	if problem != "":
		return problem
	if to == slot:
		return ""
	if exists(to):
		return "There is already a save called \"%s\"." % to
	var err := DirAccess.rename_absolute(path_of(slot), path_of(to))
	if err != OK:
		return "Could not rename it (%s)." % error_string(err)
	if is_autosave(slot):
		var d := DirAccess.open(root)
		if d != null:
			for name in d.get_directories():
				var link := path_of(name).path_join(HOME)
				if FileAccess.file_exists(link) and FileAccess.get_file_as_string(link).strip_edges() == slot:
					var link_problem := write_home(path_of(name), to)
					if link_problem != "":
						# Restore the directory and links already changed rather than leave a half rename.
						DirAccess.rename_absolute(path_of(to), path_of(slot))
						for changed in d.get_directories():
							var changed_link := path_of(changed).path_join(HOME)
							if FileAccess.file_exists(changed_link) and FileAccess.get_file_as_string(changed_link).strip_edges() == to:
								write_home(path_of(changed), slot)
						return link_problem
	return ""


func rename_target(slot: String, to: String) -> String:
	if is_autosave(slot) and not is_autosave(to):
		return to + AUTOSAVE_SUFFIX
	return to


func delete(slot: String) -> String:
	if name_problem(slot) != "" or not exists(slot):
		return "There is no save called \"%s\"." % slot
	remove_tree(path_of(slot))
	return "" if not exists(slot) else "Could not delete it."


## Delete a directory and everything under it.
static func remove_tree(path: String) -> void:
	if not DirAccess.dir_exists_absolute(path):
		return
	var d := DirAccess.open(path)
	if d == null:
		return
	d.include_hidden = true
	for f in d.get_files():
		DirAccess.remove_absolute(path.path_join(f))
	for sub in d.get_directories():
		remove_tree(path.path_join(sub))
	DirAccess.remove_absolute(path)
