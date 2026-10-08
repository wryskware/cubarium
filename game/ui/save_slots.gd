extends Control
## The save slots: save as a new slot, overwrite (confirmed), rename, delete (confirmed),
## load. Each slot shows its thumbnail, the world day and a species summary from its
## `meta.json`, read without loading the world. A save from another version stays
## listed, says it can't be loaded, and can still be deleted.

signal back

const THUMB_SIZE := Vector2(240, 120)

var shell  # main.gd
var _title: Label
var _new_row: HBoxContainer
var _name: LineEdit
var _save_new: Button
var _list: VBoxContainer
var _status: Label
var _back: Button
var _empty: Label
var _rows := {}  # slot name -> {"row", "load", "overwrite", "rename", "delete"}
var _mode := "save"


func _init() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	visible = false
	var dim := Panel.new()
	dim.theme_type_variation = &"Dim"
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right"]:
		margin.add_theme_constant_override("margin_" + side, 160)
	for side in ["top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 60)
	add_child(margin)
	var panel := PanelContainer.new()
	margin.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)
	_title = Label.new()
	_title.theme_type_variation = &"Title"
	box.add_child(_title)
	_new_row = HBoxContainer.new()
	box.add_child(_new_row)
	var label := Label.new()
	label.text = "New save"
	_new_row.add_child(label)
	_name = LineEdit.new()
	_name.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_name.placeholder_text = "a name for this save"
	_name.max_length = 40
	_name.text_submitted.connect(func(_t): _on_save_new())
	_new_row.add_child(_name)
	_save_new = Button.new()
	_save_new.text = "Save"
	_save_new.pressed.connect(_on_save_new)
	_new_row.add_child(_save_new)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	scroll.follow_focus = true
	box.add_child(scroll)
	_list = VBoxContainer.new()
	_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(_list)
	_empty = Label.new()
	_empty.text = "No saves yet."
	_empty.theme_type_variation = &"Small"
	_list.add_child(_empty)
	var bottom := HBoxContainer.new()
	box.add_child(bottom)
	_back = Button.new()
	_back.text = "Back"
	_back.custom_minimum_size = Vector2(120, 40)
	_back.pressed.connect(back.emit)
	bottom.add_child(_back)
	_status = Label.new()
	_status.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	bottom.add_child(_status)


## Show the slots: `mode` "save" (from the menu's Save) or "load".
func open(mode: String) -> void:
	_mode = mode
	_title.text = "Save" if mode == "save" else "Load"
	_status.text = ""
	visible = true
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
			for key in ["load", "overwrite", "rename", "delete"]:
				var b: Button = row[key]
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
	if visible:
		_update_busy()


func _row(s: Dictionary, has_world: bool) -> Control:
	var name: String = s["name"]
	var meta = s.get("meta")
	var loadable: bool = s.get("loadable", false)
	var row := PanelContainer.new()
	row.theme_type_variation = &"Row"
	var h := HBoxContainer.new()
	row.add_child(h)
	var thumb := TextureRect.new()
	thumb.custom_minimum_size = THUMB_SIZE
	thumb.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	thumb.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	if s.get("thumb", "") != "":
		var img := Image.load_from_file(s["thumb"])
		if img != null:
			thumb.texture = ImageTexture.create_from_image(img)
	h.add_child(thumb)
	var info := VBoxContainer.new()
	info.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	info.add_theme_constant_override("separation", 4)
	h.add_child(info)
	var title := Label.new()
	title.text = name
	title.add_theme_font_size_override("font_size", 21)
	info.add_child(title)
	var line := Label.new()
	line.theme_type_variation = &"Small"
	line.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	info.add_child(line)
	var species := Label.new()
	species.theme_type_variation = &"Small"
	species.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	info.add_child(species)
	if meta != null:
		var size: Array = meta.get("size", [0, 0, 0])
		var saved := Time.get_datetime_string_from_unix_time(int(meta.get("saved_unix_s", 0)) + _utc_offset_s(), true)
		line.text = "Day %d, %s  ·  %d × %d × %d  ·  %s, %s  ·  saved %s" % [
			meta.get("day", 0), meta.get("time_of_day", ""), size[0], size[1], size[2],
			meta.get("landform", ""), meta.get("climate", "").replace("_", " "), saved]
		species.text = species_summary(meta)
	else:
		line.text = "No details."
	if not loadable:
		var why := Label.new()
		why.theme_type_variation = &"Warn"
		why.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		var problem: String = str(s.get("problem", ""))
		var older := problem.contains("another version") or problem.contains("older build")
		why.text = "Made by an older version — can't be loaded." if older else "Can't be loaded: " + problem
		why.tooltip_text = problem
		info.add_child(why)
	var buttons := VBoxContainer.new()
	buttons.add_theme_constant_override("separation", 6)
	h.add_child(buttons)
	var r := {"row": row, "loadable": loadable}
	for item in [["load", "Load"], ["overwrite", "Overwrite"], ["rename", "Rename"], ["delete", "Delete"]]:
		var b := Button.new()
		b.text = item[1]
		b.custom_minimum_size = Vector2(130, 0)
		b.set_meta("slot_action", item[0] + ":" + name)
		buttons.add_child(b)
		r[item[0]] = b
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
