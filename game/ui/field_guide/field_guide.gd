extends Control
## **The field guide** (design/handoffs/field-guide-live-2026-10-09.md): every species with
## live data from the running world. The look is field_guide.tscn, laid out in the editor
## from ui/neon/ (round 3's Lanternjaw page): this script only fills it. Without a shell
## (opened in the editor, or by itself in a test) it leaves the scene's sample page alone.
##
## Where each part comes from:
## - the list order, swatches, kicker, role, habit and field note: game/data/field_guide.md;
## - body size, height, diet, prey and eaten-by: `session.query("guide")` (the running
##   world's config);
## - counts: `query("populations")`; the bars: `query("history")` (36 bars of 10 min over
##   the last 6 h of world time, empty where the world is younger); the clock:
##   `query("clock")`; Paused: `session.status()`.
##
## The shell (main.gd) opens it and closes it with back or Esc; "Show in world" asks the
## shell to select one living animal of the species. It asks the session about once a
## second, only while open.

signal back
signal show_in_world(species: String)

const NOTES := "res://data/field_guide.md"
const ROW := preload("res://ui/field_guide/species_row.tscn")
const CHIP := preload("res://ui/field_guide/species_chip.tscn")
const PLATE := "res://ui/field_guide/plates/%s-px.png"
const GLOW := preload("res://ui/neon/styles/glow_focus.tres")
const REFRESH_S := 1.0
const BARS := 36
const WINDOW_S := 6.0 * 3600.0
## One plate texture pixel to screen pixels: the plates are 4x point-scaled art drawn at
## 3 screen px per art pixel.
const PLATE_SCALE := 0.75
## The labels colour (round 3's --lab): food-class chips and plants without a swatch.
const LABEL_COLOUR := Color("#A595D8")
const FOODS := {"litter": "Litter", "cap_tissue": "Cap tissue", "carrion": "Carrion",
	"foliage": "Foliage", "film": "Film", "nectar": "Nectar", "fruit": "Fruit"}
var CHEVRON_CLOSED := PackedVector2Array([Vector2(0, 0), Vector2(1, 1), Vector2(2, 2), Vector2(1, 3), Vector2(0, 4)])
var CHEVRON_OPEN := PackedVector2Array([Vector2(0, 0), Vector2(1, 1), Vector2(2, 2), Vector2(3, 1), Vector2(4, 0)])

## The shell (main.gd); null in the editor or a bare test, which keeps the sample page.
var shell: Node

## Notes in file order: {id, name, swatch, kind, kicker, role, habit, habit_label, note,
## animal, plate}.
var notes: Array = []
## The selected species' id (the sim's name).
var selected := ""

var _rows := {}  # id -> species row
var _guide := {}  # id -> the guide query's entry
var _animal_index := {}  # id -> index in the history's per-lineage arrays
var _plant_index := {}  # id -> index in the history's per-species arrays
var _samples: Array = []
var _generation := -1
var _refresh_left := 0.0
var _plants_open := false
var _animals_open := true
var _populations := {}

@onready var _world_picture: Control = $World
@onready var _search: LineEdit = $Center/Page/Body/Columns/SpeciesIndex/IndexStack/SearchMargin/Search/SearchRow/SearchField
@onready var _search_box: Control = $Center/Page/Body/Columns/SpeciesIndex/IndexStack/SearchMargin/Search
@onready var _index_fade: Control = $Center/Page/Body/Columns/SpeciesIndex/IndexStack/IndexFade
@onready var _scroll: ScrollContainer = $Center/Page/Body/Columns/SpeciesIndex/IndexStack/IndexFade/IndexScroll
@onready var _list: Control = $Center/Page/Body/Columns/SpeciesIndex/IndexStack/IndexFade/IndexScroll/IndexList
@onready var _animal_rows: VBoxContainer = _list.get_node("SpeciesListMargin/SpeciesList")
@onready var _plant_rows: VBoxContainer = _list.get_node("PlantsListMargin/PlantsList")
@onready var _animals_toggle: BaseButton = _list.get_node("AnimalsHeader/Toggle")
@onready var _animals_chevron: Control = _list.get_node("AnimalsHeader/Toggle/Row/Name/Chevron")
@onready var _animals_count: Label = _list.get_node("AnimalsHeader/Toggle/Row/Count")
@onready var _fill: Control = _list.get_node("Fill")
@onready var _plants_toggle: BaseButton = _list.get_node("PlantsGroup/Toggle")
@onready var _chevron: Control = _list.get_node("PlantsGroup/Toggle/Row/Name/Chevron")
@onready var _plants_count: Label = _list.get_node("PlantsGroup/Toggle/Row/Count")
@onready var _entry: Control = $Center/Page/Body/Columns/Entry/EntryRow
@onready var _art: TextureRect = _entry.get_node("PlateColumn/Plate/PlateClip/ArtCenter/Art")
@onready var _caption: Label = _entry.get_node("PlateColumn/Plate/CaptionMargin/Caption")
@onready var _count: Label = _entry.get_node("PlateColumn/LiveCount/LiveMargin/LiveStack/LiveBody/CountColumn/Count")
@onready var _stages: Label = _entry.get_node("PlateColumn/LiveCount/LiveMargin/LiveStack/LiveBody/CountColumn/Stages")
@onready var _bars: Control = _entry.get_node("PlateColumn/LiveCount/LiveMargin/LiveStack/LiveBody/Chart/Bars")
@onready var _text: Control = _entry.get_node("TextColumn/EntryFade/EntryScroll/EntryText")
@onready var _entry_scroll: ScrollContainer = _entry.get_node("TextColumn/EntryFade/EntryScroll")
@onready var _facts: Control = _text.get_node("Facts")
@onready var _chips: Control = _facts.get_node("EatsCell/Chips")
@onready var _show: BaseButton = _entry.get_node("TextColumn/Actions/ActionRow/ShowInWorld")
@onready var _populations_button: BaseButton = _entry.get_node("TextColumn/Actions/ActionRow/Populations")
@onready var _clock: Control = $Center/Page/Top/Clock


func _ready() -> void:
	_search.text_changed.connect(func(_t): _filter())
	_search.focus_entered.connect(_search_box.queue_redraw)
	_search.focus_exited.connect(_search_box.queue_redraw)
	_search_box.draw.connect(_draw_search_focus)
	_animals_toggle.toggled.connect(_set_animals_open)
	_plants_toggle.toggled.connect(_set_plants_open)
	_show.pressed.connect(press_show)
	get_viewport().gui_focus_changed.connect(_on_focus_changed)
	if shell == null:
		return
	visible = false
	# The live world shows through the scrim; the picture is the editor's stand-in.
	_world_picture.visible = false
	notes = read_notes(NOTES)
	_build_rows()


# --- Notes --------------------------------------------------------------------------

## game/data/field_guide.md's species, in its order: a `## Name` heading, `key: value`
## lines, then the field note.
static func read_notes(path: String) -> Array:
	var out: Array = []
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		push_error("field guide: can't read %s" % path)
		return out
	var cur: Dictionary = {}
	var note: PackedStringArray = []
	for line in f.get_as_text().split("\n"):
		if line.begins_with("## "):
			if not cur.is_empty():
				cur["note"] = " ".join(note).strip_edges()
				out.append(cur)
			cur = {"name": line.substr(3).strip_edges()}
			note = []
		elif cur.is_empty():
			continue
		elif note.is_empty() and line.find(": ") > 0 and line.substr(0, line.find(": ")).is_valid_ascii_identifier():
			var k := line.substr(0, line.find(": "))
			cur[k] = line.substr(line.find(": ") + 2).strip_edges()
		elif line.strip_edges() != "":
			note.append(line.strip_edges())
	if not cur.is_empty():
		cur["note"] = " ".join(note).strip_edges()
		out.append(cur)
	for i in out.size():
		var n: Dictionary = out[i]
		n["animal"] = n.get("kind", "") == "Animal"
		n["plate"] = i + 1
		n["colour"] = Color.from_string(n.get("swatch", ""), LABEL_COLOUR)
	return out


func _note(id: String) -> Dictionary:
	for n in notes:
		if n.get("id", "") == id:
			return n
	return {}


# --- The list -----------------------------------------------------------------------

func _build_rows() -> void:
	var group := ButtonGroup.new()
	for box in [_animal_rows, _plant_rows]:
		for c in box.get_children():
			box.remove_child(c)
			c.queue_free()
	_rows.clear()
	for n in notes:
		var r = ROW.instantiate()
		r.title = n["name"]
		r.swatch = n["colour"]
		r.count = ""
		r.species = n.get("id", "")
		r.button_group = group
		r.pressed.connect(select.bind(r.species))
		(_animal_rows if n["animal"] else _plant_rows).add_child(r)
		_rows[r.species] = r
	_animals_open = true
	_set_plants_open(false)


func _set_plants_open(on: bool) -> void:
	_plants_open = on
	if _plants_toggle.button_pressed != on:
		_plants_toggle.set_pressed_no_signal(on)
	_filter()


## The Animals group collapses as Plants does (open at the start).
func _set_animals_open(on: bool) -> void:
	_animals_open = on
	if _animals_toggle.button_pressed != on:
		_animals_toggle.set_pressed_no_signal(on)
	_filter()


## Show the rows whose name holds the search text; while there is text, both groups open
## and show their matches. Group counts are the rows that match.
func _filter() -> void:
	var q := _search.text.strip_edges().to_lower()
	var shown := [0, 0]
	for n in notes:
		var r = _rows.get(n.get("id", ""))
		if r == null:
			continue
		r.visible = q == "" or String(n["name"]).to_lower().contains(q)
		if r.visible:
			shown[0 if n["animal"] else 1] += 1
	var animals_open := _animals_open or q != ""
	_animal_rows.get_parent().visible = animals_open
	_animals_chevron.pixels = CHEVRON_OPEN if animals_open else CHEVRON_CLOSED
	var open := _plants_open or q != ""
	_plant_rows.get_parent().visible = open
	_chevron.pixels = CHEVRON_OPEN if open else CHEVRON_CLOSED
	_fill.visible = not open
	_animals_count.text = str(shown[0])
	_plants_count.text = str(shown[1])


func _on_focus_changed(c: Control) -> void:
	if c == null or not visible or not _list.is_ancestor_of(c):
		return
	# Keep the focused row clear of the fades, focus glow included.
	var margin := float(maxi(_index_fade.fade, _index_fade.fade_top))
	var top := c.get_global_rect().position.y - _list.get_global_rect().position.y
	var bottom := top + c.size.y
	var v := float(_scroll.scroll_vertical)
	var h := _scroll.size.y
	if top - margin < v:
		v = top - margin
	elif bottom + margin > v + h:
		v = bottom + margin - h
	_scroll.scroll_vertical = int(clampf(v, 0.0, maxf(_list.size.y - h, 0.0)))


func _draw_search_focus() -> void:
	if _search.has_focus():
		_search_box.draw_style_box(GLOW, Rect2(Vector2.ZERO, _search_box.size))


# --- Opening and refreshing -----------------------------------------------------------

## Show it over the world on the selected species (the first, the first time).
func open() -> void:
	visible = true
	_generation = -1
	_refresh()
	if selected == "" and not notes.is_empty():
		selected = notes[0].get("id", "")
	select(selected)
	var r = _rows.get(selected)
	if r != null and r.is_visible_in_tree():
		r.grab_focus()
	else:
		_search.grab_focus()


func close() -> void:
	visible = false


func _process(delta: float) -> void:
	if not visible or shell == null:
		return
	_refresh_left -= delta
	if _refresh_left <= 0.0:
		_refresh()


func _session():
	return shell.session if shell != null else null


func _refresh() -> void:
	_refresh_left = REFRESH_S
	var s = _session()
	if s == null:
		return
	var st: Dictionary = s.status()
	var gen: int = st.get("generation", 0)
	if gen != _generation:
		_generation = gen
		_samples = []
		_guide.clear()
	if _guide.is_empty():
		# The guide arrives with the world's first publication; ask again until it has.
		_load_tables(s)
		if not _guide.is_empty() and selected != "":
			select(selected)
	_clock.get_node("Paused").visible = st.get("paused", false)
	var clock = s.query("clock", null)
	if clock is Dictionary:
		_clock.get_node("Day").text = str(int(clock.get("day", 0)))
		var hour: float = clock.get("hour", 0.0)
		_clock.get_node("Time").text = "%02d:%02d" % [int(hour) % 24, int(fmod(hour, 1.0) * 60.0)]
	var pops = s.query("populations", null)
	_populations = pops if pops is Dictionary else {}
	var since = {"since": int(_samples[-1]["tick"]) + 1} if not _samples.is_empty() else null
	var h = s.query("history", since)
	if h is Dictionary:
		_samples.append_array(h.get("samples", []))
		var now: float = _samples[-1].get("sim_s", 0.0) if not _samples.is_empty() else 0.0
		while not _samples.is_empty() and float(_samples[0].get("sim_s", 0.0)) < now - WINDOW_S - 600.0:
			_samples.pop_front()
	for n in notes:
		var r = _rows.get(n.get("id", ""))
		if r != null:
			r.count = str(_live(n.get("id", ""))["count"])
	_show_live()


func _load_tables(s) -> void:
	_guide.clear()
	var g = s.query("guide", null)
	if g is Dictionary:
		for e in g.get("animals", []) + g.get("plants", []):
			_guide[e["name"]] = e
	var table = s.query("species", null)
	if table is Dictionary:
		for a in table.get("animals", []):
			_animal_index[a["name"]] = int(a["id"])
		for p in table.get("plants", []):
			_plant_index[p["name"]] = int(p["id"])


## The live count of species `id`: {count, sub}.
func _live(id: String) -> Dictionary:
	for a in _populations.get("animals", []):
		if a.get("species", "") == id:
			var young: int = int(a.get("juveniles", 0)) + int(a.get("pups", 0)) + int(a.get("riders", 0))
			var adults: int = int(a.get("adults", 0))
			return {"count": adults + young, "sub": "%s · %s" % [_plural(adults, "adult"), _plural(young, "juvenile")]}
	for p in _populations.get("plants", []):
		if p.get("species", "") == id:
			var n: int = int(p.get("stands", 0))
			return {"count": n, "sub": _plural(n, "stand")}
	if id == "latticevine":
		var v: Dictionary = _populations.get("vines", {})
		var n: int = int(v.get("vines", 0))
		return {"count": n, "sub": "%s · %s" % [_plural(n, "vine"), _plural(int(v.get("faces", 0)), "face")]}
	return {"count": 0, "sub": ""}


static func _plural(n: int, word: String) -> String:
	return "%d %s" % [n, word if n == 1 else word + "s"]


## The 36 bars: the count at the end of each 10 min of the last 6 h of world time; -1
## (drawn empty) where the world is younger or the sim keeps no series.
func bar_values(id: String) -> PackedFloat32Array:
	var out := PackedFloat32Array()
	out.resize(BARS)
	out.fill(-1.0)
	if _samples.is_empty():
		return out
	var ai: int = _animal_index.get(id, -1)
	var pi: int = _plant_index.get(id, -1)
	if ai < 0 and pi < 0:
		return out
	var now: float = _samples[-1].get("sim_s", 0.0)
	var step := WINDOW_S / BARS
	for smp in _samples:
		var age: float = now - float(smp.get("sim_s", 0.0))
		if age < 0.0 or age >= WINDOW_S:
			continue
		var b := BARS - 1 - int(floorf(age / step))
		var v := 0.0
		if ai >= 0:
			v = float(smp["adults"][ai]) + float(smp["juveniles"][ai])
		else:
			v = float(smp["stands"][pi])
		# samples come oldest first: the newest in the bucket wins
		out[b] = v
	return out


func _show_live() -> void:
	var live := _live(selected)
	_count.text = str(live["count"])
	_stages.text = live["sub"]
	var vals := bar_values(selected)
	var top := 0.0
	for v in vals:
		top = maxf(top, v)
	_bars.baseline = 0.0
	_bars.px_per_unit = _bars.custom_minimum_size.y / top if top > 0.0 else 1.0
	_bars.values = vals
	_show.disabled = not (_note(selected).get("animal", false) and int(live["count"]) > 0)


# --- The entry ----------------------------------------------------------------------

## Show species `id`'s page.
func select(id: String) -> void:
	var n := _note(id)
	if n.is_empty():
		return
	selected = id
	var r = _rows.get(id)
	if r != null and not r.button_pressed:
		r.button_pressed = true
	var animal: bool = n["animal"]
	var g: Dictionary = _guide.get(id, {})
	var tex: Texture2D = load(PLATE % id) if ResourceLoader.exists(PLATE % id) else null
	_art.texture = tex
	_art.visible = tex != null
	if tex != null:
		_art.custom_minimum_size = tex.get_size() * PLATE_SCALE
	_caption.text = "Plate %02d · %s" % [n["plate"], "Adult" if animal else "Mature"]
	_text.get_node("Kicker").text = "%s · %s" % [n.get("kind", ""), n.get("kicker", "")]
	_text.get_node("Name").title = n["name"]
	_text.get_node("FieldNote").text = n.get("note", "")
	_fact("Role", "Role", n.get("role", ""))
	if animal:
		var size := ""
		if g.has("length_m"):
			size = "%s m long · %s m tall" % [metres(g["length_m"]), metres(g["height_m"])]
		_fact("Body", "Body", size)
	else:
		_fact("Body", "Height", "%s m" % metres(g["height_m"]) if g.get("height_m") != null else "")
	_fact("Habit", n.get("habit_label", "Habit" if animal else "Grows"), n.get("habit", ""))
	_facts.get_node("EatsLabelCell/Label").text = "Eats" if animal else "Eaten by"
	_fill_chips(g, animal)
	_entry_scroll.scroll_vertical = 0
	_show_live()


## A fact row: shown with `value`, hidden without one.
func _fact(row: String, label: String, value: String) -> void:
	var l := _facts.get_node(row + "LabelCell")
	var c := _facts.get_node(row + "Cell")
	l.visible = value != ""
	c.visible = value != ""
	l.get_node("Label").text = label
	c.get_node("Value").text = value


## Metres as round 3 writes them: two decimals at most, one at least (1.0, 0.45).
static func metres(v) -> String:
	var s := "%.2f" % float(v)
	return s.substr(0, s.length() - 1) if s.ends_with("0") else s


func _fill_chips(g: Dictionary, animal: bool) -> void:
	for c in _chips.get_children():
		_chips.remove_child(c)
		c.queue_free()
	if animal:
		for p in g.get("prey", []):
			var n := _note(p["name"])
			var title: String = n.get("name", p["name"]) + "s"
			match p.get("when"):
				"grounded": title = "Grounded " + title.to_lower()
				"at_rest": title = "Resting " + title.to_lower()
			_chip(title, n.get("colour", LABEL_COLOUR), p["name"])
		for d in g.get("diet", []):
			_chip(FOODS.get(d["food"], String(d["food"]).capitalize()), LABEL_COLOUR, "")
	else:
		for n in notes:
			if n["animal"] and g.get("eaten_by", []).has(n.get("id", "")):
				_chip(n["name"] + "s", n["colour"], n.get("id", ""))
	var any := _chips.get_child_count() > 0
	_facts.get_node("EatsLabelCell").visible = any
	_facts.get_node("EatsCell").visible = any


func _chip(title: String, colour: Color, species: String) -> void:
	var c = CHIP.instantiate()
	c.title = title
	c.swatch = colour
	c.species = species
	if species == "":
		c.focus_mode = Control.FOCUS_NONE
		c.mouse_filter = Control.MOUSE_FILTER_IGNORE
	else:
		c.pressed.connect(func(): _go_to(species))
	_chips.add_child(c)


## A species chip: that species' page, its row focused.
func _go_to(id: String) -> void:
	var n := _note(id)
	if n.is_empty():
		return
	if not n["animal"] and not _plants_open:
		_set_plants_open(true)
	if n["animal"] and not _animals_open:
		_set_animals_open(true)
	select(id)
	var r = _rows.get(id)
	if r != null and r.is_visible_in_tree():
		r.grab_focus()


## "Show in world" (Y): an animal species with a living body.
func press_show() -> void:
	if _show.disabled:
		return
	show_in_world.emit(selected)
