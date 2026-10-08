extends Control
## The new-world picker: a size, a terrain preset and a climate preset (the catalog in
## `res://data/catalog.json`, shown where the session's `presets` query lists the name),
## how much water (Low / Medium / High: the terrarium's water table at 0.375, 0.75 and
## 1.125 m), an optional seed, then 3–4 **terrain** candidates drawn in parallel — bare
## terrains, checked for room for the founders, nothing founded. Each shows its thumbnail
## (a WorldView in its own SubViewport); open one large to pan across it, reroll, or move
## the water slider to see the same terrains wetter or drier. Only the picked one is
## founded, on its own terrain, with a real progress bar; a founding that can't place the
## founders comes back here with the reason. Drawing and founding can be cancelled, and
## leaving the picker cancels them.

signal back

const WorldViews := preload("res://ui/world_views.gd")
const CATALOG := "res://data/catalog.json"
const CARD_VIEW := Vector2(320, 160)

var shell  # main.gd
var catalog := {}
var sizes := []
var terrains := []
var climates := []

var _size: OptionButton
var _terrain: OptionButton
var _climate: OptionButton
var _size_note: Label
var _terrain_note: Label
var _climate_note: Label
var _seed: LineEdit
var _water: HSlider
var _water_note: Label
var _water_box: Control
## The water steps the session offers (`query("spec")`): name and water_level_m.
var water_levels := [{"name": "low", "water_level_m": 0.375}, {"name": "medium", "water_level_m": 0.75},
	{"name": "high", "water_level_m": 1.125}]
var _water_ok := false  # the chosen landform has a water table
var _generate: Button
var _cancel: Button
var _back: Button
var _status: Label
var _grid: GridContainer
var _cards := []
var _views := {}  # candidate index -> its whole view, kept for opening large
var _started_ms := 0
var _batch_key := ""
var _ready_ms := {}
var _generating := false
var _large: Control
var _large_holder: Control
var _large_title: Label
var _large_pick: Button
var _large_back: Button
var _large_index := -1
var _founding := false
var _found_index := -1
var _found_box: PanelContainer
var _found_title: Label
var _found_phase: Label
var _found_bar: ProgressBar
var _found_detail: Label
var _found_cancel: Button


func _init() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	visible = false
	var dim := Panel.new()
	dim.theme_type_variation = &"Dim"
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 40)
	add_child(margin)
	var panel := PanelContainer.new()
	margin.add_child(panel)
	var h := HBoxContainer.new()
	h.add_theme_constant_override("separation", 24)
	panel.add_child(h)

	var left := VBoxContainer.new()
	left.custom_minimum_size = Vector2(340, 0)
	h.add_child(left)
	var title := Label.new()
	title.text = "New world"
	title.theme_type_variation = &"Title"
	left.add_child(title)
	_size = _option(left, "Size")
	_size_note = _note(left)
	_terrain = _option(left, "Terrain")
	_terrain_note = _note(left)
	_climate = _option(left, "Climate")
	_climate_note = _note(left)
	var seed_label := Label.new()
	seed_label.text = "Seed"
	left.add_child(seed_label)
	_seed = LineEdit.new()
	_seed.placeholder_text = "blank: drawn for each candidate"
	left.add_child(_seed)
	_water_box = VBoxContainer.new()
	left.add_child(_water_box)
	var water_label := Label.new()
	water_label.text = "Water"
	_water_box.add_child(water_label)
	_water = HSlider.new()
	_water.min_value = 0
	_water.max_value = 2
	_water.step = 1
	_water.tick_count = 3
	_water.ticks_on_borders = true
	_water.value = 1
	_water.custom_minimum_size = Vector2(0, 32)
	_water.value_changed.connect(func(_v): _water_changed())
	_water_box.add_child(_water)
	var steps := HBoxContainer.new()
	_water_box.add_child(steps)
	for i in 3:
		var l := Label.new()
		l.text = ["Low", "Medium", "High"][i]
		l.theme_type_variation = &"Small"
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		l.horizontal_alignment = [HORIZONTAL_ALIGNMENT_LEFT, HORIZONTAL_ALIGNMENT_CENTER,
			HORIZONTAL_ALIGNMENT_RIGHT][i]
		steps.add_child(l)
	_water_note = _note(_water_box)
	var spacer := Control.new()
	spacer.size_flags_vertical = Control.SIZE_EXPAND_FILL
	left.add_child(spacer)
	_status = Label.new()
	_status.theme_type_variation = &"Small"
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	left.add_child(_status)
	_generate = Button.new()
	_generate.text = "Generate"
	_generate.custom_minimum_size = Vector2(0, 44)
	_generate.pressed.connect(func(): generate())
	left.add_child(_generate)
	_cancel = Button.new()
	_cancel.text = "Cancel"
	_cancel.custom_minimum_size = Vector2(0, 40)
	_cancel.visible = false
	_cancel.pressed.connect(func(): cancel())
	left.add_child(_cancel)
	_back = Button.new()
	_back.text = "Back"
	_back.custom_minimum_size = Vector2(0, 40)
	_back.pressed.connect(back.emit)
	left.add_child(_back)

	var column := VBoxContainer.new()
	column.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	h.add_child(column)
	_build_founding(column)
	var right := ScrollContainer.new()
	right.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	right.follow_focus = true
	column.add_child(right)
	_grid = GridContainer.new()
	_grid.columns = 2
	_grid.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right.add_child(_grid)

	_size.item_selected.connect(func(_i): _notes(); _sync_water())
	_terrain.item_selected.connect(func(_i): _notes(); _sync_water())
	_climate.item_selected.connect(func(_i): _notes())
	_build_large()


func _option(parent: Control, label: String) -> OptionButton:
	var l := Label.new()
	l.text = label
	parent.add_child(l)
	var o := OptionButton.new()
	o.custom_minimum_size = Vector2(0, 40)
	parent.add_child(o)
	return o


func _note(parent: Control) -> Label:
	var l := Label.new()
	l.theme_type_variation = &"Small"
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	parent.add_child(l)
	return l


## The founding panel over the candidates: what is being founded, its stage, a progress
## bar driven by the session's own progress, the try, and Cancel.
func _build_founding(parent: Control) -> void:
	_found_box = PanelContainer.new()
	_found_box.theme_type_variation = &"Row"
	_found_box.visible = false
	parent.add_child(_found_box)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 6)
	_found_box.add_child(box)
	_found_title = Label.new()
	_found_title.theme_type_variation = &"Title"
	box.add_child(_found_title)
	_found_phase = Label.new()
	box.add_child(_found_phase)
	_found_bar = ProgressBar.new()
	_found_bar.custom_minimum_size = Vector2(0, 16)
	_found_bar.show_percentage = false
	box.add_child(_found_bar)
	_found_detail = Label.new()
	_found_detail.theme_type_variation = &"Small"
	box.add_child(_found_detail)
	_found_cancel = Button.new()
	_found_cancel.text = "Cancel founding"
	_found_cancel.custom_minimum_size = Vector2(200, 40)
	_found_cancel.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_found_cancel.pressed.connect(func(): cancel())
	box.add_child(_found_cancel)


func _build_large() -> void:
	_large = Control.new()
	_large.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_large.visible = false
	add_child(_large)
	var dim := Panel.new()
	dim.theme_type_variation = &"Dim"
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_large.add_child(dim)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 24)
	_large.add_child(margin)
	var panel := PanelContainer.new()
	margin.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)
	_large_title = Label.new()
	_large_title.theme_type_variation = &"Title"
	box.add_child(_large_title)
	_large_holder = Control.new()
	_large_holder.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_large_holder.clip_contents = true
	box.add_child(_large_holder)
	var row := HBoxContainer.new()
	box.add_child(row)
	_large_back = Button.new()
	_large_back.text = "Back"
	_large_back.custom_minimum_size = Vector2(120, 44)
	_large_back.pressed.connect(close_large)
	row.add_child(_large_back)
	_large_pick = Button.new()
	_large_pick.text = "Pick this world"
	_large_pick.custom_minimum_size = Vector2(200, 44)
	_large_pick.pressed.connect(func(): _pick(_large_index))
	row.add_child(_large_pick)
	var hint := Label.new()
	hint.theme_type_variation = &"Small"
	hint.text = ("Pan: drag, arrows, WASD or the left stick." if WorldViews.is_real()
		else "Pan: drag, WASD or the right stick. Zoom: wheel.")
	row.add_child(hint)


# --- Opening ----------------------------------------------------------------------------

## Load the catalog and fill the choices from what the session lists.
func setup() -> void:
	var text := FileAccess.get_file_as_string(CATALOG)
	var parsed = JSON.parse_string(text)
	catalog = parsed if parsed is Dictionary else {}
	sizes = catalog.get("sizes", [])
	var listed = shell.session.query("presets", null)
	if not (listed is Dictionary) or listed.has("error"):
		listed = {"terrain": [], "climate": []}
	terrains = _offered(catalog.get("terrain", []), listed.get("terrain", []))
	climates = _offered(catalog.get("climate", []), listed.get("climate", []))
	_size.clear()
	for s in sizes:
		_size.add_item(s["label"])
	_terrain.clear()
	for t in terrains:
		_terrain.add_item(t["label"])
	_climate.clear()
	for c in climates:
		_climate.add_item(c["label"])
	_notes()
	_sync_water()


## The catalog's entries the session lists, in the catalog's order, then any listed name
## the catalog lacks (under its own name), so a new preset shows up without a data edit.
static func _offered(entries: Array, listed: Array) -> Array:
	var names := listed.map(func(p): return p["name"])
	var out := entries.filter(func(e): return e["name"] in names)
	var known := out.map(func(e): return e["name"])
	for n in names:
		if not n in known:
			out.append({"name": n, "label": n, "description": ""})
	return out


func open() -> void:
	visible = true
	if _large.visible:
		_large_back.grab_focus()
	elif _cards.is_empty():
		_generate.grab_focus()
	else:
		focus_first()


## Leaving the picker: whatever is being drawn or founded is cancelled.
func leave() -> void:
	if _generating or _founding:
		cancel()


## Stop the candidates being drawn and the founding in flight (the session says which
## ended how; ready candidates stay).
func cancel() -> void:
	shell.session.cancel()
	if _generating:
		_generating = false
		_status.text = "Cancelled."
	_refresh_controls()


func focus_first() -> void:
	for card in _cards:
		if not card["open"].disabled:
			card["open"].grab_focus()
			return
	_generate.grab_focus()


## Esc or B: close the large candidate if it is open (true), else nothing to close here.
func back_one() -> bool:
	if _large.visible:
		close_large()
		return true
	return false


func _notes() -> void:
	var s = _selected(sizes, _size)
	_size_note.text = s.get("description", "") if s else ""
	var t = _selected(terrains, _terrain)
	_terrain_note.text = t.get("description", "") if t else ""
	var c = _selected(climates, _climate)
	_climate_note.text = c.get("description", "") if c else ""


func _selected(list: Array, option: OptionButton):
	var i := option.selected
	return list[i] if i >= 0 and i < list.size() else null


func select(size_id: String, terrain := "", climate := "") -> void:
	for i in sizes.size():
		if sizes[i]["id"] == size_id:
			_size.select(i)
	for i in terrains.size():
		if terrains[i]["name"] == terrain:
			_terrain.select(i)
	for i in climates.size():
		if climates[i]["name"] == climate:
			_climate.select(i)
	_notes()
	_sync_water()


# --- Water ------------------------------------------------------------------------------

## Put the slider on the step matching the chosen config's own water table, or Medium if
## none matches; hide it for a landform without one. Doesn't regenerate anything.
func _sync_water() -> void:
	var base := spec(false)
	base.erase("seed")
	var info = shell.session.query("spec", base)
	_water_ok = info is Dictionary and info.get("landform", "") == "terrarium"
	_water_box.visible = _water_ok
	if not _water_ok:
		return
	var listed = info.get("water_levels", [])
	if listed is Array and listed.size() == 3:
		water_levels = listed
	var own = info.get("water_level_m")
	var step := 1
	for i in water_levels.size():
		if own != null and absf(float(water_levels[i]["water_level_m"]) - float(own)) < 1e-6:
			step = i
	_water.set_value_no_signal(step)
	_water_text()


func water_level_m() -> float:
	return float(water_levels[int(_water.value)]["water_level_m"])


func _water_text() -> void:
	_water_note.text = "The water table %.3f m over the yard's ground." % water_level_m()


## The slider moved: the same terrains again at the new level (their seeds kept), or
## nothing yet when no candidates are shown.
func _water_changed() -> void:
	_water_text()
	if _cards.is_empty() or _founding:
		return
	var seeds := []
	for c in shell.session.candidates():
		seeds.append(c.get("seed") if c.get("state") == "ready" else null)
	if seeds.is_empty():
		return
	var refused: String = shell.session.regenerate_candidates(spec(), seeds)
	if refused != "":
		_status.text = "Can't make these: " + refused
		return
	_start_batch(seeds.size(), "Drawing the same %d terrains with %s water…" % [seeds.size(),
		["low", "medium", "high"][int(_water.value)]])


# --- Generating -------------------------------------------------------------------------

## The spec for the chosen size, terrain, climate and seed. A preset the size's config
## already uses is not sent, so the file's own landform and climate tables stand.
func spec(with_water := true) -> Dictionary:
	var s = _selected(sizes, _size)
	var root := ProjectSettings.globalize_path("res://").path_join("..").simplify_path()
	var dims: Array = s["size"].map(func(v): return int(v))  # JSON numbers are floats
	var out := {"config": root.path_join(s["config"]), "size": dims}
	var t = _selected(terrains, _terrain)
	if t and t["name"] != s.get("terrain", ""):
		out["terrain"] = t["name"]
	var c = _selected(climates, _climate)
	if c and c["name"] != s.get("climate", ""):
		out["climate"] = c["name"]
	if with_water and _water_ok:
		out["water_level_m"] = water_level_m()
	var seed := _seed.text.strip_edges()
	if seed != "" and seed_problem(seed) == "":
		out["seed"] = seed  # a String: seeds are unsigned 64-bit, Godot's ints are signed
	return out


## Why `text` can't be sent as a seed, or "". Seeds cross the binding as decimal Strings,
## so any unsigned 64-bit value goes, the drawn ones above 2^63 included.
static func seed_problem(text: String) -> String:
	const MAX := "18446744073709551615"
	for ch in text:
		if ch < "0" or ch > "9":
			return "A seed is a whole number, 0 or more."
	if text == "":
		return "A seed is a whole number, 0 or more."
	var digits := text.lstrip("0")
	if digits.length() > MAX.length() or (digits.length() == MAX.length() and digits > MAX):
		return "Seeds go up to %s." % MAX
	return ""


## Found the size's number of candidates (the catalog's), or `count` when given.
func generate(count := 0) -> String:
	var s = _selected(sizes, _size)
	if s == null:
		return "no size to make"
	if count <= 0:
		count = int(s.get("candidates", 4))
	var seed := _seed.text.strip_edges()
	if seed != "" and seed_problem(seed) != "":
		_status.text = seed_problem(seed)
		return seed_problem(seed)
	if _founding:
		return "a world is being founded"
	var refused: String = shell.session.generate_candidates(spec(), count)
	if refused != "":
		push_warning("new world: " + refused)
		_status.text = "Can't make these: " + refused
		return refused
	_start_batch(count, "Drawing %d terrains…" % count)
	return ""


## Cards for a batch the session has just started.
func _start_batch(count: int, says: String) -> void:
	var s = _selected(sizes, _size)
	_clear_cards()
	_batch_key = "terrain/%s/%s" % [s["id"], count]
	_started_ms = Time.get_ticks_msec()
	_generating = true
	var aspect := float(s["size"][0]) / float(s["size"][1])
	_grid.columns = 2 if aspect <= 3.0 else 1
	for i in count:
		_cards.append(_card(i, aspect))
	_generate.text = "Reroll"
	_status.text = says
	_refresh_controls()


func _clear_cards() -> void:
	close_large()
	for card in _cards:
		card["root"].queue_free()
	_cards.clear()
	_views.clear()
	_ready_ms.clear()


## A candidate's card: its view (as wide as the column, at the world's own aspect),
## its progress, a population line, and Open and Pick.
func _card(i: int, aspect: float) -> Dictionary:
	var root := PanelContainer.new()
	root.theme_type_variation = &"Row"
	root.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_grid.add_child(root)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 6)
	root.add_child(box)
	var holder := Control.new()
	holder.custom_minimum_size = CARD_VIEW
	holder.clip_contents = true
	holder.resized.connect(func():
		var h := maxf(holder.size.x / aspect, 120.0)
		if absf(holder.custom_minimum_size.y - h) > 1.0:
			holder.custom_minimum_size = Vector2(CARD_VIEW.x, h))
	box.add_child(holder)
	var bg := ColorRect.new()
	bg.color = Color(0.05, 0.06, 0.085)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	holder.add_child(bg)
	var state := Label.new()
	box.add_child(state)
	var bar := ProgressBar.new()
	bar.custom_minimum_size = Vector2(0, 10)
	bar.show_percentage = false
	box.add_child(bar)
	var summary := Label.new()
	summary.theme_type_variation = &"Small"
	summary.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(summary)
	var row := HBoxContainer.new()
	box.add_child(row)
	var open_b := Button.new()
	open_b.text = "Open"
	open_b.disabled = true
	open_b.custom_minimum_size = Vector2(110, 0)
	open_b.pressed.connect(func(): open_large(i))
	row.add_child(open_b)
	var pick_b := Button.new()
	pick_b.text = "Pick"
	pick_b.disabled = true
	pick_b.custom_minimum_size = Vector2(110, 0)
	pick_b.pressed.connect(func(): _pick(i))
	row.add_child(pick_b)
	return {"root": root, "holder": holder, "state": state, "bar": bar, "summary": summary,
		"open": open_b, "pick": pick_b, "shown": false}


## Buttons as the work in hand allows: nothing new while a world is founded, Cancel while
## anything is in flight.
func _refresh_controls() -> void:
	_generate.disabled = _founding
	_water.editable = not _founding
	_cancel.visible = _generating and not _founding
	for card in _cards:
		var ready: bool = card.get("ready", false)
		card["pick"].disabled = _founding or not ready or not card.get("feasible", true)
		card["open"].disabled = not ready
	_large_pick.disabled = _founding or (_large_index >= 0 and _large_index < _cards.size()
		and _cards[_large_index]["pick"].disabled)


func _process(_delta: float) -> void:
	if _founding:
		_follow_founding()
	if _cards.is_empty():
		return
	var list: Array = shell.session.candidates()
	var now := Time.get_ticks_msec()
	var elapsed := (now - _started_ms) / 1000.0
	var estimate: float = shell.founding_estimate_s(_batch_key)
	var waiting := 0
	for c in list:
		var i: int = c["index"]
		if i >= _cards.size():
			continue
		var card: Dictionary = _cards[i]
		var seed_text := "seed %s" % str(c["seed"]) if c.get("seed") != null else ""
		match c["state"]:
			"generating":
				waiting += 1
				card["state"].text = "Drawing the terrain…  %d s" % int(elapsed)
				var bar: ProgressBar = card["bar"]
				if estimate > 0.0:
					bar.indeterminate = false
					bar.value = minf(elapsed / estimate, 0.97) * 100.0
				else:
					bar.indeterminate = true
			"ready":
				if not _ready_ms.has(i):
					_ready_ms[i] = now
				var took: float = (_ready_ms[i] - _started_ms) / 1000.0
				card["state"].text = "Ready (%s), %d s" % [seed_text, int(took)]
				card["bar"].indeterminate = false
				card["bar"].value = 100.0
				if not card["shown"]:
					_show_candidate(i, c)
			"failed":
				card["state"].text = "No terrain"
				card["bar"].indeterminate = false
				card["bar"].value = 0.0
				card["summary"].text = str(c.get("error", ""))
			"cancelled":
				card["state"].text = "Cancelled"
				card["bar"].indeterminate = false
				card["bar"].value = 0.0
	if _generating and waiting == 0 and list.size() > 0:
		_generating = false
		shell.record_founding_s(_batch_key, elapsed)
		var ready := list.filter(func(x): return x["state"] == "ready").size()
		_status.text = "%d of %d terrains in %d s. Pick one to found it." % [ready, list.size(), int(elapsed)]
		_refresh_controls()
		if get_viewport().gui_get_focus_owner() == null or get_viewport().gui_get_focus_owner() == _generate:
			focus_first()


func _show_candidate(i: int, c: Dictionary) -> void:
	var card: Dictionary = _cards[i]
	card["shown"] = true
	var view: Dictionary = shell.session.candidate_view(i)
	if view.is_empty():
		return
	_views[i] = view
	WorldViews.make_framed(view, card["holder"])
	card["ready"] = true
	card["feasible"] = c.get("feasible") != false
	card["terrain"] = c.get("terrain", "")
	card["summary"].text = _terrain_line(c)
	_refresh_controls()


## What a terrain card says under its picture: bare until picked, and what the
## feasibility check found.
static func _terrain_line(c: Dictionary) -> String:
	var line := "Bare terrain: the plants and animals are founded when you pick it."
	if c.get("feasible") == false:
		line = "The founders can't all be placed on this terrain."
	for n in c.get("notes", []):
		line += "\n" + str(n)
	return line


func open_large(i: int) -> void:
	if not _views.has(i):
		return
	close_large()
	_large_index = i
	WorldViews.make_framed(_views[i], _large_holder, true)
	_large_title.text = "Candidate %d  ·  %s" % [i + 1, _cards[i]["state"].text]
	_large.visible = true
	_refresh_controls()
	_large_pick.grab_focus()


func close_large() -> void:
	if not _large.visible:
		return
	_large.visible = false
	for c in _large_holder.get_children():
		c.queue_free()
	if _large_index >= 0 and _large_index < _cards.size():
		_cards[_large_index]["open"].grab_focus()
	_large_index = -1


## The open large view's WorldView, or null.
func large_view() -> Node:
	if not _large.visible or _large_holder.get_child_count() == 0:
		return null
	return WorldViews.framed_view(_large_holder.get_child(0))


func is_large_open() -> bool:
	return _large.visible


func ready_count() -> int:
	return _views.size()


## Found candidate `i` on its own terrain; the panel follows it until it runs, fails or
## is cancelled.
func _pick(i: int) -> void:
	var problem: String = shell.found_candidate(i)
	if problem != "":
		_status.text = "Can't pick it: " + problem
		return
	close_large()
	_founding = true
	_found_index = i
	_found_title.text = "Founding candidate %d" % (i + 1)
	_found_phase.text = "Starting…"
	_found_bar.value = 0.0
	_found_detail.text = ""
	_found_box.visible = true
	_status.text = ""
	_refresh_controls()
	_found_cancel.grab_focus()


func is_founding() -> bool:
	return _founding


## The session's founding, onto the panel; and what happens when it ends.
func _follow_founding() -> void:
	var f: Dictionary = shell.session.founding()
	match f.get("state", ""):
		"founding":
			var label: String = f.get("phase_label", "")
			_found_phase.text = (label if label != "" else "Starting") + "…"
			_found_bar.value = clampf(float(f.get("progress", 0.0)), 0.0, 1.0) * 100.0
			var detail := "seed %s  ·  %d s" % [str(f.get("seed", "")), int(f.get("elapsed_s", 0.0))]
			if int(f.get("attempt", 0)) > 1:
				detail += "  ·  placing the founders again, try %d of %d" % [f["attempt"], f["attempts"]]
			_found_detail.text = detail
		"done":
			_end_founding()
			_clear_cards()
			_generate.text = "Generate"
			_status.text = ""
			shell.on_founded(f)
		"failed":
			_end_founding()
			_status.text = str(f.get("error", "The founding failed."))
		"cancelled":
			_end_founding()
			_status.text = "Founding cancelled."


func _end_founding() -> void:
	_founding = false
	_found_index = -1
	_found_box.visible = false
	_refresh_controls()

