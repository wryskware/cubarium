extends PanelContainer
## The inspect panel: one animal's stats and life history from `query("animal", {id})`,
## refreshed about once a second, with a plain "died" state once it is gone. The shell
## highlights and follows the animal while the panel is open.

signal closed

const REFRESH_S := 1.0
const EVENTS_SHOWN := 40

var shell  # main.gd
var animal_id := -1
var dead := false
var last := {}  # the newest answer
var _since := 0.0
var _title: Label
var _close: Button
var _state: Label
var _stats: GridContainer
var _life: VBoxContainer


func _init() -> void:
	visible = false
	anchor_left = 1.0
	anchor_right = 1.0
	anchor_top = 0.0
	anchor_bottom = 1.0
	offset_left = -400
	offset_right = -16
	offset_top = 16
	offset_bottom = -16
	var box := VBoxContainer.new()
	add_child(box)
	var head := HBoxContainer.new()
	box.add_child(head)
	_title = Label.new()
	_title.theme_type_variation = &"Title"
	_title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_title.clip_text = true
	head.add_child(_title)
	_close = Button.new()
	_close.text = "Close"
	_close.pressed.connect(close)
	head.add_child(_close)
	_state = Label.new()
	_state.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(_state)
	_stats = GridContainer.new()
	_stats.columns = 2
	_stats.add_theme_constant_override("v_separation", 4)
	box.add_child(_stats)
	var life_title := Label.new()
	life_title.text = "Life"
	life_title.add_theme_font_size_override("font_size", 21)
	box.add_child(life_title)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	box.add_child(scroll)
	_life = VBoxContainer.new()
	_life.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_life.add_theme_constant_override("separation", 2)
	scroll.add_child(_life)


func open(id: int) -> void:
	animal_id = id
	dead = false
	last = {}
	visible = true
	_since = 0.0
	refresh()
	_close.grab_focus()


func close() -> void:
	if not visible:
		return
	visible = false
	animal_id = -1
	closed.emit()


func _process(delta: float) -> void:
	if not visible:
		return
	_since += delta
	if _since >= REFRESH_S:
		_since = 0.0
		refresh()


func refresh() -> void:
	if animal_id < 0:
		return
	var q = shell.session.query("animal", {"id": animal_id})
	if not (q is Dictionary) or q.has("error"):
		dead = true
		_title.text = "#%d" % animal_id
		_state.text = "Gone: nothing is remembered of it any more."
		return
	last = q
	var body = q.get("animal")
	var life = q.get("life")
	var label := ""
	if body is Dictionary:
		label = body.get("label", "")
	elif life is Dictionary:
		label = life.get("label", "")
	_title.text = "%s #%d" % [label, animal_id]
	var death = life.get("death") if life is Dictionary else null
	dead = body == null
	if dead:
		var line := "Died"
		if death is Dictionary:
			line += " (%s) at tick %d" % [str(death.get("cause", "unknown")).replace("_", " "), int(death.get("tick", 0))]
		_state.text = line + "."
	else:
		_state.text = "%s, %s" % [body.get("stage", ""), body.get("activity", body.get("state", ""))]
	_fill_stats(body)
	_fill_life(life)


func _fill_stats(body) -> void:
	for c in _stats.get_children():
		c.queue_free()
	if not (body is Dictionary):
		return
	var rows := [
		["Age", _duration(body.get("age_s", 0.0)) + " of " + _duration(body.get("lifespan_s", 0.0))],
		["Energy", "%.3f" % body.get("energy", 0.0)],
		["Reserve", "%.3f" % body.get("reserve", 0.0)],
		["Fullness", "%d %%" % int(100.0 * body.get("fullness", 0.0))],
		["Wound", "%.2f" % body.get("wound", 0.0)],
		["Size", "%.2f" % body.get("size", 0.0)],
		["Where", "%.1f, %.1f, %.1f m" % [body.get("x_m", 0.0), body.get("y_m", 0.0), body.get("z_m", 0.0)]],
	]
	if body.get("gestating", false):
		rows.append(["Gestating", "yes"])
	if body.get("pack") != null:
		rows.append(["Pack", str(body["pack"])])
	if body.get("riding") != null:
		rows.append(["Riding", "#%s" % body["riding"]])
	for r in rows:
		var k := Label.new()
		k.text = r[0]
		k.theme_type_variation = &"Small"
		_stats.add_child(k)
		var v := Label.new()
		v.text = r[1]
		_stats.add_child(v)


func _fill_life(life) -> void:
	for c in _life.get_children():
		c.queue_free()
	if not (life is Dictionary):
		_line("No events recorded yet.", true)
		return
	var origin = life.get("origin")
	if origin is Dictionary:
		_line("%s  %s" % [_when(origin), _event_text(origin)], false)
	if life.get("parent") != null:
		_line("Parent: #%s" % life["parent"], true)
	var offspring: Array = life.get("offspring", [])
	if offspring.size() > 0:
		_line("Offspring: %s" % ", ".join(offspring.map(func(o): return "#%s" % o)), true)
	var events: Array = life.get("events", [])
	if int(life.get("dropped", 0)) > 0:
		_line("(%d older events not kept)" % int(life["dropped"]), true)
	var newest := events.slice(maxi(events.size() - EVENTS_SHOWN, 0))
	newest.reverse()
	var death = life.get("death")
	if death is Dictionary:
		_line("%s  %s" % [_when(death), _event_text(death)], false)
	for e in newest:
		_line("%s  %s" % [_when(e), _event_text(e)], false)


func _line(text: String, small: bool) -> void:
	var l := Label.new()
	l.text = text
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	if small:
		l.theme_type_variation = &"Small"
	_life.add_child(l)


func _when(e: Dictionary) -> String:
	return "t%d" % int(e.get("tick", 0))


## One event in words, from its kind and whatever fields it carries.
func _event_text(e: Dictionary) -> String:
	var kind: String = e.get("kind", "?")
	match kind:
		"observed":
			return "first seen, %s, aged %s" % [e.get("stage", ""), _duration(e.get("age_s", 0.0))]
		"born":
			return "born (%s)" % str(e.get("how", "")).replace("_", " ")
		"meal":
			var foods: Array = e.get("foods", [])
			var what := ", ".join(foods.map(func(f): return str(f.get("food", "?")) if f is Dictionary else str(f)))
			return "ate %s (%s bout, %s)" % [what, e.get("bout", ""), _duration(e.get("duration_s", 0.0))]
		"hunt":
			return "struck at #%s" % e.get("prey", "?")
		"kill":
			return "killed #%s (%s)" % [e.get("prey", "?"), e.get("prey_species", "")]
		"injured":
			return "injured, wound %.2f%s" % [e.get("wound", 0.0), " by #%s" % e["by"] if e.get("by") != null else ""]
		"gave_birth":
			return "gave birth to %d" % (e.get("young", []) as Array).size()
		"laid_eggs":
			return "laid %d eggs" % int(e.get("eggs", 0))
		"died":
			return "died: %s%s" % [str(e.get("cause", "")).replace("_", " "), " (by #%s)" % e["by"] if e.get("by") != null else ""]
	return kind.replace("_", " ")


static func _duration(s) -> String:
	var t := float(s)
	if t < 90.0:
		return "%d s" % int(t)
	if t < 5400.0:
		return "%d min" % int(t / 60.0)
	return "%.1f h" % (t / 3600.0)
