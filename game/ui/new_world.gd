extends Control
## The new-world picker: a size, a terrain preset and a climate preset (the catalog in
## `res://data/catalog.json`, shown where the session's `presets` query lists the name),
## how much water (Low / Medium / High: the terrarium's water table at 0.375, 0.75 and
## 1.125 m), an optional seed, then 3–4 **terrain** candidates drawn in parallel — bare
## terrains, checked for room for the founders, nothing founded. Each shows its thumbnail
## (a WorldView in its own SubViewport); open one large to pan across it, reroll, or move
## the water slider to see the same terrains wetter or drier. Only the picked one is
## founded, on its own terrain, with a real progress row; a founding that can't place the
## founders comes back here with the reason. Drawing and founding can be cancelled, and
## leaving the picker cancels them.
##
## The look is new_world.tscn (round 4's New world and Founding views), laid out in the
## editor from ui/neon/; each terrain is a terrain_card.tscn. Size, terrain and climate are
## NeonArrows, water a NeonDotSlider, progress NeonDotProgress rows. This script only fills
## and drives them. Without a shell (opened in the editor, or by itself in a test) it leaves
## the scene's sample page alone.
##
## Keys and pad, as round 4's hints say: on a terrain card, A (Enter, or the Found button)
## founds it, Y (G) opens it large (a double click too), X rerolls; B (Esc) goes back. A
## click selects a card; Found this world founds the selected one.

signal back

const WorldViews := preload("res://ui/world_views.gd")
const CARD := preload("res://ui/terrain_card.tscn")
const CATALOG := "res://data/catalog.json"
## The thumbnail's height when its width isn't known yet.
const CARD_VIEW := Vector2(336, 168)

var shell  # main.gd
var catalog := {}
var sizes := []
var terrains := []
var climates := []

## The water steps the session offers (`query("spec")`): name and water_level_m.
var water_levels := [{"name": "low", "water_level_m": 0.375}, {"name": "medium", "water_level_m": 0.75},
	{"name": "high", "water_level_m": 1.125}]
var _water_ok := false  # the chosen landform has a water table
var _cards := []
var _views := {}  # candidate index -> what its WorldView was shown, kept for opening large
var _selected := -1  # the card Found this world founds: the last one focused
var _started_ms := 0
var _batch_key := ""
var _ready_ms := {}
var _generating := false
var _large_index := -1
var _founding := false
var _found_index := -1

@onready var _world_picture: Control = $World
@onready var _page: Control = $Center
@onready var _form: Control = %Form
@onready var _size: NeonArrows = %Size
@onready var _terrain: NeonArrows = %Terrain
@onready var _climate: NeonArrows = %Climate
@onready var _size_note: Label = %SizeNote
@onready var _terrain_note: Label = %TerrainNote
@onready var _climate_note: Label = %ClimateNote
@onready var _water_box: Control = %WaterField
@onready var _water: NeonDotSlider = %Water
@onready var _water_note: Label = %WaterNote
@onready var _seed_box: Control = %SeedBox
@onready var _seed: LineEdit = %Seed
@onready var _header: Label = %Header
@onready var _cards_scroll: Control = %Cards
@onready var _grid: GridContainer = %Grid
@onready var _status: Label = %Status
@onready var _found: BaseButton = %Found
@onready var _generate: BaseButton = %Generate
@onready var _generate_label: Label = %GenerateLabel
@onready var _cancel: BaseButton = %Cancel
@onready var _found_box: Control = %Founding
@onready var _found_phase: Label = %Phase
@onready var _found_bar: NeonDotProgress = %FoundBar
@onready var _found_detail: Label = %FoundDetail
@onready var _found_cancel: BaseButton = %FoundCancel
@onready var _hints := {"open": %OpenHint, "found": %FoundHint, "reroll": %RerollHint,
	"back": %BackHint, "cancel": %CancelHint}
@onready var _reroll_hint: Label = %RerollHintLabel
@onready var _large: Control = %Large
@onready var _large_holder: Control = %LargeHolder
@onready var _large_title: Label = %LargeTitle
@onready var _large_pick: BaseButton = %LargePick
@onready var _large_back: BaseButton = %LargeBack


func _ready() -> void:
	_seed.focus_entered.connect(_seed_box.queue_redraw)
	_seed.focus_exited.connect(_seed_box.queue_redraw)
	_seed_box.draw.connect(_draw_seed_focus)
	if shell == null:
		return
	visible = false
	# The live world shows through the scrim; the picture is the editor's stand-in.
	_world_picture.visible = false
	for c in _grid.get_children():
		_grid.remove_child(c)
		c.queue_free()
	_status.text = ""
	_water.changed.connect(func(_i): _water_changed())
	_size.changed.connect(func(_i): _notes(); _sync_water())
	_terrain.changed.connect(func(_i): _notes(); _sync_water())
	_climate.changed.connect(func(_i): _notes())
	_found.pressed.connect(func(): _pick(_selected))
	_generate.pressed.connect(func(): generate())
	_cancel.pressed.connect(func(): cancel())
	_found_cancel.pressed.connect(func(): cancel())
	_large_back.pressed.connect(close_large)
	_large_pick.pressed.connect(func(): _pick(_large_index))
	%PanHint.text = ("Wheel: zoom. Pan: drag, arrows, WASD or left stick. Home: overview." if WorldViews.is_real()
		else "Pan: drag, WASD or the right stick. Zoom: wheel.")
	_refresh_controls()


func _draw_seed_focus() -> void:
	if _seed.has_focus():
		_seed_box.draw_style_box(preload("res://ui/neon/styles/glow_focus.tres"), Rect2(Vector2.ZERO, _seed_box.size))


## Y (or G) opens the focused card large; the pad's X rerolls.
func _unhandled_input(event: InputEvent) -> void:
	if shell == null or not visible or _large.visible or _founding or event.is_echo():
		return
	if InputMap.has_action("shell_guide") and event.is_action_pressed("shell_guide"):
		var i := _card_index(get_viewport().gui_get_focus_owner())
		if i >= 0 and _views.has(i):
			get_viewport().set_input_as_handled()
			open_large(i)
	elif event is InputEventJoypadButton and event.pressed and event.button_index == JOY_BUTTON_X:
		get_viewport().set_input_as_handled()
		if not _generate.disabled:
			generate()


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
	_size.options = PackedStringArray(sizes.map(func(s): return s["label"]))
	_size.index = 0
	_terrain.options = PackedStringArray(terrains.map(func(t): return t["label"]))
	_terrain.index = 0
	_climate.options = PackedStringArray(climates.map(func(c): return c["label"]))
	_climate.index = 0
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
		if card.get("ready", false):
			card["root"].grab_focus()
			return
	_generate.grab_focus()


## Esc or B: close the large candidate if it is open (true), else nothing to close here.
func back_one() -> bool:
	if _large.visible:
		close_large()
		return true
	return false


func _notes() -> void:
	var s = _selected_entry(sizes, _size)
	_size_note.text = s.get("description", "") if s else ""
	var t = _selected_entry(terrains, _terrain)
	_terrain_note.text = t.get("description", "") if t else ""
	var c = _selected_entry(climates, _climate)
	_climate_note.text = c.get("description", "") if c else ""
	for note in [_size_note, _terrain_note, _climate_note]:
		note.visible = note.text != ""


func _selected_entry(list: Array, choice: NeonArrows):
	var i := choice.index
	return list[i] if i >= 0 and i < list.size() else null


func select(size_id: String, terrain := "", climate := "") -> void:
	for i in sizes.size():
		if sizes[i]["id"] == size_id:
			_size.index = i
	for i in terrains.size():
		if terrains[i]["name"] == terrain:
			_terrain.index = i
	for i in climates.size():
		if climates[i]["name"] == climate:
			_climate.index = i
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
	_water.index = step
	_water_text()


func water_level_m() -> float:
	return float(water_levels[_water.index]["water_level_m"])


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
		["low", "medium", "high"][_water.index]])


# --- Generating -------------------------------------------------------------------------

## The spec for the chosen size, terrain, climate and seed. A preset the size's config
## already uses is not sent, so the file's own landform and climate tables stand.
func spec(with_water := true) -> Dictionary:
	var s = _selected_entry(sizes, _size)
	var root := ProjectSettings.globalize_path("res://").path_join("..").simplify_path()
	var dims: Array = s["size"].map(func(v): return int(v))  # JSON numbers are floats
	var out := {"config": root.path_join(s["config"]), "size": dims}
	var t = _selected_entry(terrains, _terrain)
	if t and t["name"] != s.get("terrain", ""):
		out["terrain"] = t["name"]
	var c = _selected_entry(climates, _climate)
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
	var s = _selected_entry(sizes, _size)
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
	var s = _selected_entry(sizes, _size)
	_clear_cards()
	_batch_key = "terrain/%s/%s" % [s["id"], count]
	_started_ms = Time.get_ticks_msec()
	_generating = true
	var aspect := float(s["size"][0]) / float(s["size"][1])
	_grid.columns = 2 if aspect <= 3.0 else 1
	for i in count:
		_cards.append(_card(i, aspect))
	_status.text = says
	_refresh_controls()


func _clear_cards() -> void:
	close_large()
	for card in _cards:
		card["root"].queue_free()
	_cards.clear()
	_views.clear()
	_ready_ms.clear()
	_selected = -1


## A candidate's card (terrain_card.tscn): its view (as wide as the card, at the world's
## own aspect), its name and state, a small progress row while it is drawn, and a line for
## anything worth saying. A or Enter founds it, a click selects it, a double click opens it.
func _card(i: int, aspect: float) -> Dictionary:
	var root: BaseButton = CARD.instantiate()
	_grid.add_child(root)
	var holder: Control = root.get_node("%Holder")
	root.get_node("%Sample").free()
	holder.custom_minimum_size = Vector2(0, CARD_VIEW.y)
	holder.resized.connect(func():
		var h := maxf(holder.size.x / aspect, 120.0)
		if absf(holder.custom_minimum_size.y - h) > 1.0:
			holder.custom_minimum_size = Vector2(0, h))
	(root.get_node("%Name") as Label).text = "Terrain %d" % (i + 1)
	root.pressed.connect(func(): _pick(i))
	root.focus_entered.connect(func():
		_selected = i
		_refresh_controls())
	root.gui_input.connect(func(e: InputEvent):
		if e is InputEventMouseButton and e.double_click and e.button_index == MOUSE_BUTTON_LEFT:
			open_large(i))
	var card := {"root": root, "holder": holder, "state": root.get_node("%State"),
		"bar": root.get_node("%Bar"), "summary": root.get_node("%Info"), "shown": false}
	_set_card_state(card, "Drawing", true)
	return card


func _set_card_state(card: Dictionary, state: String, drawing: bool) -> void:
	card["state"].text = state
	card["bar"].visible = drawing
	card["holder"].modulate.a = 0.35 if drawing else 1.0


func _set_card_note(card: Dictionary, text: String) -> void:
	card["summary"].text = text
	card["summary"].visible = text != ""


## Whether card `i` can be founded now.
func _pickable(i: int) -> bool:
	return not _founding and i >= 0 and i < _cards.size() and _cards[i].get("ready", false) \
		and _cards[i].get("feasible", true)


## Whether card `i` has a picture to open large.
func can_open(i: int) -> bool:
	return _views.has(i)


func _card_index(node: Node) -> int:
	for i in _cards.size():
		if _cards[i]["root"] == node:
			return i
	return -1


## Buttons and hints as the work in hand allows: nothing new while a world is founded,
## Cancel while anything is in flight.
func _refresh_controls() -> void:
	var has_cards := not _cards.is_empty()
	_generate.disabled = _founding
	_generate.visible = not _founding
	_generate_label.text = " Reroll " if has_cards else " Generate "
	_reroll_hint.text = "Reroll" if has_cards else "Generate"
	_found.visible = has_cards and not _founding
	_found.disabled = not _pickable(_selected)
	_cancel.visible = _generating and not _founding
	_found_cancel.visible = _founding
	_found_box.visible = _founding
	_cards_scroll.visible = not _founding
	_header.text = ("Founding terrain %d" % (_found_index + 1)) if _founding else "Terrains"
	_lock_form(_founding)
	_hints["open"].visible = has_cards and not _founding
	_hints["found"].visible = has_cards and not _founding
	_hints["reroll"].visible = not _founding
	_hints["back"].visible = not _founding
	_hints["cancel"].visible = _founding
	_large_pick.disabled = _founding or not _pickable(_large_index)


## While a world is founded the form dims (round 4's Founding view) and takes no input.
func _lock_form(locked: bool) -> void:
	_form.modulate.a = 0.4 if locked else 1.0
	for c: Control in [_size, _terrain, _climate, _water]:
		c.focus_mode = Control.FOCUS_NONE if locked else Control.FOCUS_ALL
		c.mouse_filter = Control.MOUSE_FILTER_IGNORE if locked else Control.MOUSE_FILTER_STOP
	_seed.editable = not locked
	_seed.focus_mode = Control.FOCUS_NONE if locked else Control.FOCUS_ALL
	_seed.mouse_filter = Control.MOUSE_FILTER_IGNORE if locked else Control.MOUSE_FILTER_STOP


func _process(_delta: float) -> void:
	if shell == null:
		return
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
		match c["state"]:
			"generating":
				waiting += 1
				_set_card_state(card, "Drawing", true)
				var bar: NeonDotProgress = card["bar"]
				if estimate > 0.0:
					bar.indeterminate = false
					bar.value = minf(elapsed / estimate, 0.97)
				else:
					bar.indeterminate = true
				_set_card_note(card, "%d s" % int(elapsed))
			"ready":
				if not _ready_ms.has(i):
					_ready_ms[i] = now
				_set_card_state(card, "Ready", false)
				card["bar"].indeterminate = false
				card["bar"].value = 1.0
				if not card["shown"]:
					_set_card_note(card, "")
					_show_candidate(i, c)
				elif not card.get("framed", false):
					_follow_thumbnail(card)
			"failed":
				_set_card_state(card, "No terrain", false)
				card["bar"].indeterminate = false
				card["bar"].value = 0.0
				_set_card_note(card, str(c.get("error", "")))
			"cancelled":
				_set_card_state(card, "Cancelled", false)
				card["bar"].indeterminate = false
				card["bar"].value = 0.0
				_set_card_note(card, "")
	if _generating and waiting == 0 and list.size() > 0:
		_generating = false
		shell.record_founding_s(_batch_key, elapsed)
		var ready := list.filter(func(x): return x["state"] == "ready").size()
		_status.text = "%d of %d terrains in %d s. Pick one to found it." % [ready, list.size(), int(elapsed)]
		_refresh_controls()
		if get_viewport().gui_get_focus_owner() == null or get_viewport().gui_get_focus_owner() == _generate:
			focus_first()


## Give a ready card its thumbnail. The renderer's WorldView needs only which candidate
## (`candidate_ref`, nothing of the world converted on this thread); the stub draws the
## whole view. Tried again next frame if the candidate isn't there to show yet.
func _show_candidate(i: int, c: Dictionary) -> void:
	var card: Dictionary = _cards[i]
	var view: Dictionary = (shell.session.candidate_ref(i) if WorldViews.is_real()
		else shell.session.candidate_view(i))
	if view.is_empty():
		return
	card["shown"] = true
	_views[i] = view
	card["view"] = WorldViews.framed_view(WorldViews.make_framed(view, card["holder"]))
	card["ready"] = true
	card["feasible"] = c.get("feasible") != false
	card["terrain"] = c.get("terrain", "")
	card["seed"] = str(c.get("seed", "")) if c.get("seed") != null else ""
	_set_card_note(card, _terrain_line(c))
	_refresh_controls()


## A shown thumbnail until its first frame: if its renderer gave up, say why on the card
## rather than leave it blank.
func _follow_thumbnail(card: Dictionary) -> void:
	var wv = card.get("view")
	if wv == null or not wv.has_method("problem"):
		card["framed"] = true
		return
	if wv.frames_shown > 0:
		card["framed"] = true
		return
	var problem: String = wv.problem()
	if problem != "":
		card["framed"] = true
		_set_card_note(card, "No picture: " + problem)


## What a terrain card says under its name: nothing for a plain bare terrain (Found this
## world founds it), else what the feasibility check found.
static func _terrain_line(c: Dictionary) -> String:
	var lines: PackedStringArray = []
	if c.get("feasible") == false:
		lines.append("The founders can't all be placed on this terrain.")
	for n in c.get("notes", []):
		lines.append(str(n))
	return "\n".join(lines)


func open_large(i: int) -> void:
	if not _views.has(i):
		return
	close_large()
	_large_index = i
	WorldViews.make_framed(_views[i], _large_holder, true)
	var seed: String = _cards[i].get("seed", "")
	_large_title.text = "Terrain %d" % (i + 1) + ("  ·  seed %s" % seed if seed != "" else "")
	_large.visible = true
	_page.visible = false
	_refresh_controls()
	_large_pick.grab_focus()


func close_large() -> void:
	if not _large.visible:
		return
	_large.visible = false
	_page.visible = true
	for c in _large_holder.get_children():
		c.queue_free()
	if _large_index >= 0 and _large_index < _cards.size():
		_cards[_large_index]["root"].grab_focus()
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
## is cancelled. Nothing happens for a card that can't be founded (still drawing, or the
## founders don't fit; its card says which).
func _pick(i: int) -> void:
	if not _pickable(i):
		return
	var problem: String = shell.found_candidate(i)
	if problem != "":
		_status.text = "Can't pick it: " + problem
		return
	close_large()
	_founding = true
	_found_index = i
	_found_phase.text = "Starting…"
	_found_bar.value = 0.0
	_found_detail.text = ""
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
			_found_bar.value = clampf(float(f.get("progress", 0.0)), 0.0, 1.0)
			var detail := "seed %s  ·  %d s" % [str(f.get("seed", "")), int(f.get("elapsed_s", 0.0))]
			if int(f.get("attempt", 0)) > 1:
				detail += "  ·  placing the founders again, try %d of %d" % [f["attempt"], f["attempts"]]
			_found_detail.text = detail
		"done":
			_end_founding()
			_clear_cards()
			_status.text = ""
			_refresh_controls()
			shell.on_founded(f)
		"failed":
			_end_founding()
			_status.text = str(f.get("error", "The founding failed."))
		"cancelled":
			_end_founding()
			_status.text = "Founding cancelled."


func _end_founding() -> void:
	var had_focus := _found_cancel.has_focus()
	_founding = false
	_found_index = -1
	_refresh_controls()
	if had_focus and visible:
		focus_first()
