extends Control
## The new-world picker: a size, a terrain preset and a climate preset (the catalog in
## `res://data/catalog.json`, shown where the session's `presets` query lists the name),
## an optional seed, then 3–4 candidates founded in parallel. Each candidate shows its
## progress and, once ready, its thumbnail: a WorldView in its own SubViewport. Open one
## large to pan across it; pick one, or reroll.

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
var _generate: Button
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
	_back = Button.new()
	_back.text = "Back"
	_back.custom_minimum_size = Vector2(0, 40)
	_back.pressed.connect(back.emit)
	left.add_child(_back)

	var right := ScrollContainer.new()
	right.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	right.follow_focus = true
	h.add_child(right)
	_grid = GridContainer.new()
	_grid.columns = 2
	_grid.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right.add_child(_grid)

	_size.item_selected.connect(func(_i): _notes())
	_terrain.item_selected.connect(func(_i): _notes())
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
	hint.text = "Pan: drag, WASD or the right stick. Zoom: wheel."
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


# --- Generating -------------------------------------------------------------------------

## The spec for the chosen size, terrain, climate and seed. A preset the size's config
## already uses is not sent, so the file's own landform and climate tables stand.
func spec() -> Dictionary:
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
	var seed := _seed.text.strip_edges()
	if seed != "" and seed_problem(seed) == "":
		out["seed"] = int(seed)
	return out


## Why `text` can't be sent as a seed, or "". The binding passes integers as signed 64-bit,
## so a drawn seed above 2^63 − 1 (shown unsigned) can't be typed back in yet.
static func seed_problem(text: String) -> String:
	const MAX := "9223372036854775807"
	if not text.is_valid_int() or text.begins_with("-") or text.begins_with("+"):
		return "A seed is a whole number, 0 or more."
	var digits := text.lstrip("0")
	if digits.length() > MAX.length() or (digits.length() == MAX.length() and digits > MAX):
		return "Seeds above %s can't be passed to the session yet." % MAX
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
	var refused: String = shell.session.generate_candidates(spec(), count)
	if refused != "":
		push_warning("new world: " + refused)
		_status.text = "Can't make these: " + refused
		return refused
	_clear_cards()
	_batch_key = "%s/%s" % [s["id"], count]
	_started_ms = Time.get_ticks_msec()
	_generating = true
	var aspect := float(s["size"][0]) / float(s["size"][1])
	_grid.columns = 2 if aspect <= 3.0 else 1
	for i in count:
		_cards.append(_card(i, aspect))
	_generate.text = "Reroll"
	_status.text = "Founding %d candidates…" % count
	return ""


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


func _process(_delta: float) -> void:
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
		var seed_text := "seed %s" % String.num_uint64(c["seed"]) if c.get("seed") != null else ""
		var summary = c.get("summary")
		if summary is Dictionary and summary.has("seed"):
			seed_text = "seed %s" % String.num_uint64(summary["seed"])
		match c["state"]:
			"generating":
				waiting += 1
				card["state"].text = "Founding…  %d s" % int(elapsed)
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
				card["state"].text = "Failed"
				card["bar"].indeterminate = false
				card["bar"].value = 0.0
				card["summary"].text = str(c.get("error", ""))
	if _generating and waiting == 0 and list.size() > 0:
		_generating = false
		shell.record_founding_s(_batch_key, elapsed)
		var ready := list.filter(func(x): return x["state"] == "ready").size()
		_status.text = "%d of %d ready in %d s." % [ready, list.size(), int(elapsed)]
		if get_viewport().gui_get_focus_owner() == null or get_viewport().gui_get_focus_owner() == _generate:
			focus_first()


func _show_candidate(i: int, c: Dictionary) -> void:
	var card: Dictionary = _cards[i]
	card["shown"] = true
	var view: Dictionary = shell.session.candidate_view(i)
	if view.is_empty():
		return
	_views[i] = view
	var framed := WorldViews.make_framed(view)
	framed.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	card["holder"].add_child(framed)
	card["open"].disabled = false
	card["pick"].disabled = false
	card["summary"].text = _populations_line(c.get("populations"))


func _populations_line(pops) -> String:
	if not (pops is Dictionary):
		return ""
	var animals := 0
	var kinds := 0
	for a in pops.get("animals", []):
		animals += int(a.get("alive", 0)) + int(a.get("eggs", 0))
		if int(a.get("alive", 0)) + int(a.get("eggs", 0)) > 0:
			kinds += 1
	var stands := 0
	for p in pops.get("plants", []):
		stands += int(p.get("stands", 0))
	return "%d animals and eggs in %d lineages  ·  %d plant stands" % [animals, kinds, stands]


func open_large(i: int) -> void:
	if not _views.has(i):
		return
	close_large()
	_large_index = i
	var framed := WorldViews.make_framed(_views[i], true)
	framed.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_large_holder.add_child(framed)
	_large_title.text = "Candidate %d  ·  %s" % [i + 1, _cards[i]["state"].text]
	_large.visible = true
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


func _pick(i: int) -> void:
	var problem: String = shell.promote_candidate(i)
	if problem != "":
		_status.text = "Can't pick it: " + problem
		return
	_clear_cards()
	_generate.text = "Generate"
	_status.text = ""

