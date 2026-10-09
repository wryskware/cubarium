extends PanelContainer
## The inspect panel: one animal's stats and life history from `query("animal", {id})`,
## refreshed about once a second, with a plain "died" state once it is gone. The shell
## highlights and follows the animal while the panel is open.
##
## The look is inspect_panel.tscn, laid out in the editor from ui/neon/ (the field guide's
## facts, label and value): this script only fills it. Every fact row is in the scene and
## shows only with a value. The life list's sample lines are its templates: the first
## Event* (when, what) and Note* lines are copied for each line shown. Without a shell
## (opened in the editor, or by itself) the scene's sample animal stays.

signal closed

const REFRESH_S := 1.0
const EVENTS_SHOWN := 40
## The fact rows in the scene (Facts/<Key>LabelCell and <Key>Cell), in order.
const FACTS := ["Age", "Energy", "Reserve", "Fullness", "Wound", "Size", "Where", "Gestating", "Pack", "Riding"]

var shell  # main.gd
var animal_id := -1
var dead := false
var last := {}  # the newest answer
var _since := 0.0
var _event_line: Control
var _note_line: Label

@onready var _stack: Control = $Margin/Stack
@onready var _kicker: Label = _stack.get_node("Kicker")
@onready var _title: NeonTitle = _stack.get_node("Name")
@onready var _state: Label = _stack.get_node("State")
@onready var _stats: GridContainer = _stack.get_node("Facts")
@onready var _life: VBoxContainer = _stack.get_node("LifeFade/LifeScroll/Life")
@onready var _life_scroll: ScrollContainer = _stack.get_node("LifeFade/LifeScroll")
@onready var _close: BaseButton = _stack.get_node("Actions/Close")


func _ready() -> void:
	_close.pressed.connect(close)
	for c in _life.get_children():
		if _event_line == null and c.name.begins_with("Event"):
			_event_line = c.duplicate()
		elif _note_line == null and c.name.begins_with("Note"):
			_note_line = c.duplicate()
	if shell == null:
		return
	visible = false
	_clear_life()


func _notification(what: int) -> void:
	# The two line templates live outside the tree.
	if what == NOTIFICATION_PREDELETE:
		for t in [_event_line, _note_line]:
			if is_instance_valid(t):
				t.free()


func open(id: int) -> void:
	animal_id = id
	dead = false
	last = {}
	visible = true
	_since = 0.0
	refresh()
	_life_scroll.scroll_vertical = 0
	_close.grab_focus()


func close() -> void:
	if not visible:
		return
	visible = false
	animal_id = -1
	closed.emit()


func _process(delta: float) -> void:
	if not visible or shell == null:
		return
	_since += delta
	if _since >= REFRESH_S:
		_since = 0.0
		refresh()


func refresh() -> void:
	if animal_id < 0:
		return
	_kicker.text = "Animal · #%d" % animal_id
	var q = shell.session.query("animal", {"id": animal_id})
	if not (q is Dictionary) or q.has("error"):
		dead = true
		_title.title = "#%d" % animal_id
		_state.text = "Gone: nothing is remembered of it any more."
		_fill_stats(null)
		return
	last = q
	var body = q.get("animal")
	var life = q.get("life")
	var label := ""
	if body is Dictionary:
		label = body.get("label", "")
	elif life is Dictionary:
		label = life.get("label", "")
	# The species as the field guide names it ("Ripple-snail"): the sim's label, capitalised.
	_title.title = label.left(1).to_upper() + label.substr(1) if label != "" else "#%d" % animal_id
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
	var values := {}
	if body is Dictionary:
		values = {
			"Age": _duration(body.get("age_s", 0.0)) + " of " + _duration(body.get("lifespan_s", 0.0)),
			"Energy": "%.3f" % body.get("energy", 0.0),
			"Reserve": "%.3f" % body.get("reserve", 0.0),
			"Fullness": "%d %%" % int(100.0 * body.get("fullness", 0.0)),
			"Wound": "%.2f" % body.get("wound", 0.0),
			"Size": "%.2f" % body.get("size", 0.0),
			"Where": "%.1f, %.1f, %.1f m" % [body.get("x_m", 0.0), body.get("y_m", 0.0), body.get("z_m", 0.0)],
		}
		if body.get("gestating", false):
			values["Gestating"] = "yes"
		if body.get("pack") != null:
			values["Pack"] = str(body["pack"])
		if body.get("riding") != null:
			values["Riding"] = "#%s" % body["riding"]
	for key in FACTS:
		var v: String = values.get(key, "")
		var l := _stats.get_node(key + "LabelCell")
		var c := _stats.get_node(key + "Cell")
		l.visible = v != ""
		c.visible = v != ""
		c.get_node("Value").text = v


func _clear_life() -> void:
	for c in _life.get_children():
		_life.remove_child(c)
		c.queue_free()


func _fill_life(life) -> void:
	_clear_life()
	if not (life is Dictionary):
		_note("No events recorded yet.")
		return
	var origin = life.get("origin")
	if origin is Dictionary:
		_event(origin)
	if life.get("parent") != null:
		_note("Parent: #%s" % life["parent"])
	var offspring: Array = life.get("offspring", [])
	if offspring.size() > 0:
		_note("Offspring: %s" % ", ".join(offspring.map(func(o): return "#%s" % o)))
	var events: Array = life.get("events", [])
	if int(life.get("dropped", 0)) > 0:
		_note("(%d older events not kept)" % int(life["dropped"]))
	var newest := events.slice(maxi(events.size() - EVENTS_SHOWN, 0))
	newest.reverse()
	var death = life.get("death")
	if death is Dictionary:
		_event(death)
	for e in newest:
		_event(e)


## One event line: when, then what, from the scene's first Event line.
func _event(e: Dictionary) -> void:
	var line: Control = _event_line.duplicate()
	line.get_node("When").text = _when(e)
	line.get_node("What").text = _event_text(e)
	_life.add_child(line)


## One note line (parent, offspring, events not kept), from the scene's first Note line.
func _note(text: String) -> void:
	var line: Label = _note_line.duplicate()
	line.text = text
	_life.add_child(line)


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
