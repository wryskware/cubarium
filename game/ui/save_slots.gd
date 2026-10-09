extends Control
## The save slots: save as a new slot, overwrite (confirmed), rename, delete (confirmed),
## load. Each slot shows its thumbnail, the world day and a species summary from its
## `meta.json`, read without loading the world. A save from another version stays
## listed, says it can't be loaded, and can still be deleted.
##
## The look is save_slots.tscn and one save_slot_row.tscn per slot, laid out in the editor
## from ui/neon/ (the field guide's frame, scrim, list rows, search box and buttons): this
## script only fills them. The row holding focus shows as the selected list row. Without
## a shell (opened in the editor, or by itself) the scene's three sample slots stay.

signal back

const ROW := preload("res://ui/save_slot_row.tscn")
const GLOW := preload("res://ui/neon/styles/glow_focus.tres")
const ACTIONS := ["load", "overwrite", "rename", "delete"]

var shell  # main.gd
var _rows := {}  # slot name -> {"row", "load", "overwrite", "rename", "delete", "loadable"}
var _mode := "save"

@onready var _world: Control = $World
@onready var _stack: Control = $Center/Page/Body/Margin/Stack
@onready var _title: NeonTitle = _stack.get_node("Title")
@onready var _new_row: Control = _stack.get_node("NewMargin")
@onready var _name_box: Control = _stack.get_node("NewMargin/NewRow/NameBox")
@onready var _name: LineEdit = _stack.get_node("NewMargin/NewRow/NameBox/Name")
@onready var _save_new: BaseButton = _stack.get_node("NewMargin/NewRow/SaveNew")
@onready var _fade: Control = _stack.get_node("ListFade")
@onready var _scroll: ScrollContainer = _stack.get_node("ListFade/ListScroll")
@onready var _list: VBoxContainer = _stack.get_node("ListFade/ListScroll/List")
@onready var _empty: Label = _list.get_node("Empty")
@onready var _back: BaseButton = _stack.get_node("BottomMargin/Bottom/Back")
@onready var _status: Label = _stack.get_node("BottomMargin/Bottom/Status")


func _ready() -> void:
	_name.text_submitted.connect(func(_t): _on_save_new())
	# The name field draws no focus of its own: its box glows, as the guide's search box does.
	_name.focus_entered.connect(_name_box.queue_redraw)
	_name.focus_exited.connect(_name_box.queue_redraw)
	_name_box.draw.connect(_draw_name_focus)
	_save_new.pressed.connect(_on_save_new)
	_back.pressed.connect(back.emit)
	get_viewport().gui_focus_changed.connect(_on_focus_changed)
	if shell == null:
		return
	visible = false
	# The live world shows through the scrim; the picture is the editor's stand-in.
	_world.visible = false
	_status.text = ""
	for c in _list.get_children():
		if c != _empty:
			_list.remove_child(c)
			c.queue_free()


func _draw_name_focus() -> void:
	if _name.has_focus():
		_name_box.draw_style_box(GLOW, Rect2(Vector2.ZERO, _name_box.size))


## The row holding focus is the selected one, and stays clear of the list's fades, focus
## glow included (as the field guide's species list does).
func _on_focus_changed(c: Control) -> void:
	if c == null or not visible:
		return
	for r in _rows.values():
		r["row"].set_pressed_no_signal(r["row"].is_ancestor_of(c))
	if not _list.is_ancestor_of(c):
		return
	var row: Control = c
	while row.get_parent() != _list:
		row = row.get_parent()
	var margin := float(maxi(_fade.fade, _fade.fade_top))
	var top := row.get_global_rect().position.y - _list.get_global_rect().position.y
	var bottom := top + row.size.y
	var v := float(_scroll.scroll_vertical)
	var h := _scroll.size.y
	if top - margin < v:
		v = top - margin
	elif bottom + margin > v + h:
		v = bottom + margin - h
	_scroll.scroll_vertical = int(clampf(v, 0.0, maxf(_list.size.y - h, 0.0)))


## Show the slots: `mode` "save" (from the menu's Save) or "load".
func open(mode: String) -> void:
	_mode = mode
	_title.title = "Save" if mode == "save" else "Load"
	_status.text = ""
	visible = true
	_scroll.scroll_vertical = 0
	refresh()


func refresh() -> void:
	var keep_focus := get_viewport().gui_get_focus_owner() if is_inside_tree() else null
	var focus_meta := ""
	if keep_focus != null and keep_focus.has_meta("slot_action"):
		focus_meta = keep_focus.get_meta("slot_action")
	var has_world: bool = shell.has_world()
	_new_row.visible = has_world
	if _name.text == "" or shell.saves.exists(_name.text):
		_name.text = shell.saves.next_name()
	for c in _list.get_children():
		if c != _empty:
			_list.remove_child(c)
			c.queue_free()
	_rows.clear()
	var slots: Array = shell.saves.list(shell.session)
	_empty.visible = slots.is_empty()
	for s in slots:
		_list.add_child(_row(s, has_world))
	_update_busy()
	if focus_meta != "":
		for row in _rows.values():
			for key in ACTIONS:
				var b: BaseButton = row[key]
				if b.get_meta("slot_action") == focus_meta and not b.disabled and b.visible:
					b.grab_focus()
					return
	focus_first()


func focus_first() -> void:
	if _mode == "save" and _new_row.visible:
		_save_new.grab_focus()
		return
	for row in _rows.values():
		if not row["load"].disabled:
			row["load"].grab_focus()
			return
	if _new_row.visible:
		_save_new.grab_focus()
	else:
		_back.grab_focus()


## Say something on the status line.
func say(text: String) -> void:
	_status.text = text


func _update_busy() -> void:
	var busy: String = shell.busy()
	_save_new.disabled = busy != ""
	for name in _rows:
		var r = _rows[name]
		r["load"].disabled = busy != "" or not r["loadable"]
		r["overwrite"].disabled = busy != "" or not shell.has_world()
		r["rename"].disabled = busy != ""
		r["delete"].disabled = busy != ""


func _process(_delta: float) -> void:
	if visible and shell != null:
		_update_busy()


func _row(s: Dictionary, has_world: bool) -> Control:
	var name: String = s["name"]
	var meta = s.get("meta")
	var loadable: bool = s.get("loadable", false)
	var row: Control = ROW.instantiate()
	var info := row.get_node("Margin/Columns/Info")
	var thumb: TextureRect = row.get_node("Margin/Columns/Thumb/Picture")
	thumb.texture = null
	if s.get("thumb", "") != "":
		var img := Image.load_from_file(s["thumb"])
		if img != null:
			thumb.texture = ImageTexture.create_from_image(img)
	info.get_node("Name").text = name
	var line: Label = info.get_node("Details")
	var species: Label = info.get_node("Species")
	if meta != null:
		var size: Array = meta.get("size", [0, 0, 0])
		var saved := Time.get_datetime_string_from_unix_time(int(meta.get("saved_unix_s", 0)) + _utc_offset_s(), true)
		line.text = "Day %d, %s  ·  %d × %d × %d  ·  %s, %s  ·  saved %s" % [
			meta.get("day", 0), meta.get("time_of_day", ""), size[0], size[1], size[2],
			meta.get("landform", ""), meta.get("climate", "").replace("_", " "), saved]
		species.text = species_summary(meta)
	else:
		line.text = "No details."
		species.visible = false
	var why: Label = info.get_node("Why")
	why.visible = not loadable
	if not loadable:
		var problem: String = str(s.get("problem", ""))
		var older := problem.contains("another version") or problem.contains("older build")
		why.text = "Made by an older version — can't be loaded." if older else "Can't be loaded: " + problem
		why.tooltip_text = problem
	var r := {"row": row, "loadable": loadable}
	var actions := info.get_node("ActionsMargin/Actions")
	for key in ACTIONS:
		var b: BaseButton = actions.get_node(String(key).capitalize())
		b.set_meta("slot_action", key + ":" + name)
		r[key] = b
	r["load"].visible = loadable
	r["overwrite"].visible = has_world
	r["load"].pressed.connect(func(): shell.load_slot(name))
	r["overwrite"].pressed.connect(func(): _overwrite(name))
	r["rename"].pressed.connect(func(): _rename(name))
	r["delete"].pressed.connect(func(): _delete(name))
	_rows[name] = r
	return row


## "38 animals: 8 littershredder, 8 frondgrazer, … · 227 plant stands".
func species_summary(meta: Dictionary) -> String:
	var animals: Array = meta.get("animals", [])
	var plants: Array = meta.get("plants", [])
	var alive := animals.filter(func(a): return a[1] > 0)
	alive.sort_custom(func(a, b): return a[1] > b[1])
	var total := 0
	for a in alive:
		total += int(a[1])
	var stands := 0
	for p in plants:
		stands += int(p[1])
	var names := alive.slice(0, 4).map(func(a): return "%d %s" % [a[1], shell.species_label(a[0])])
	var more := "" if alive.size() <= 4 else ", +%d more" % (alive.size() - 4)
	var head := "%d animals" % total if total > 0 else "no animals"
	if names.size() > 0:
		head += ": " + ", ".join(names) + more
	return "%s  ·  %d plant stands" % [head, stands]


func _utc_offset_s() -> int:
	return int(Time.get_time_zone_from_system().get("bias", 0)) * 60


func _on_save_new() -> void:
	var name := _name.text.strip_edges()
	if shell.saves.exists(name):
		_overwrite(name)
		return
	var problem: String = shell.saves.name_problem(name)
	if problem != "":
		say(problem)
		return
	say(shell.save_slot(name))


func _overwrite(name: String) -> void:
	shell.dialog.ask("Overwrite \"%s\" with the world as it is now?" % name, "Overwrite",
		func(_t): return shell.save_slot(name))


func _rename(name: String) -> void:
	shell.dialog.ask("Rename \"%s\" to:" % name, "Rename", func(to):
		var problem: String = shell.saves.rename(name, to.strip_edges())
		if problem == "":
			shell.renamed_slot(name, to.strip_edges())
			refresh()
		return problem, true, name)


func _delete(name: String) -> void:
	shell.dialog.ask("Delete \"%s\"? This can't be undone." % name, "Delete", func(_t):
		var problem: String = shell.saves.delete(name)
		if problem == "":
			shell.deleted_slot(name)
			say("Deleted \"%s\"." % name)
			refresh()
		return problem)
