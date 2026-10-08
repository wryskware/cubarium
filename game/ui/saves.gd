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

var root := ""
var staging := ""


func _init(root_dir: String) -> void:
	root = root_dir.rstrip("/")
	staging = root + ".staging"
	DirAccess.make_dir_recursive_absolute(root)


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


## Why `slot` can't be a slot name, or "".
func name_problem(slot: String) -> String:
	if slot.strip_edges() != slot or slot.is_empty():
		return "A name can't be empty or start or end with a space."
	if slot.length() > MAX_NAME:
		return "A name is at most %d characters." % MAX_NAME
	if slot.begins_with(".") or not slot.is_valid_filename():
		return "A name can't start with a dot or hold any of  / \\ : * ? \" < > |"
	return ""


## The first free "World N".
func next_name() -> String:
	var n := 1
	while exists("World %d" % n):
		n += 1
	return "World %d" % n


## A clean staging directory for `slot`, to hand the session's `save`.
func begin(slot: String) -> String:
	var dir := staging.path_join(slot)
	remove_tree(dir)
	DirAccess.make_dir_recursive_absolute(dir)
	return dir


## Move a finished save from staging into its slot, replacing what was there, with its
## thumbnail. "" or why not.
func commit(slot: String, thumb: Image) -> String:
	var from := staging.path_join(slot)
	if not DirAccess.dir_exists_absolute(from):
		return "the save went missing before it could be put in its slot"
	if thumb != null and not thumb.is_empty():
		thumb.save_png(from.path_join(THUMB))
	var to := path_of(slot)
	remove_tree(to)
	var err := DirAccess.rename_absolute(from, to)
	return "" if err == OK else "could not move the save into its slot (%s)" % error_string(err)


func rename(slot: String, to: String) -> String:
	var problem := name_problem(to)
	if problem != "":
		return problem
	if to == slot:
		return ""
	if exists(to):
		return "There is already a save called \"%s\"." % to
	var err := DirAccess.rename_absolute(path_of(slot), path_of(to))
	return "" if err == OK else "Could not rename it (%s)." % error_string(err)


func delete(slot: String) -> String:
	if slot.is_empty() or not exists(slot):
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
