extends Control
## **Dev metrics** (design/handoffs/dev-metrics-panel-2026-10-08.md): the study recorder's
## per-minute rows from `session.query("vitals")`, the same numbers `voxel_census vitals=`
## writes to `vitals.csv` / `plants_vitals.csv` and the training harness reads. A
## developer panel: raw vitals only, none of score.py's terms. The dev theme as it is
## (`theme.tres`, type variations only); the real UI language restyles it later.
##
## Three views: **Animals** (one row per lineage at the chosen minute, columns grouped and
## each group toggled on or off), **Plants** (one row per species), **Charts** (a lineage
## and a column over the ring, and every lineage's population on one chart). Every column
## header's tooltip is the column's meaning and unit, served with the rows. The shell
## (main.gd) opens it from the pause menu or F3 and closes it with back or Esc.

signal back

const Chart := preload("res://ui/dev/metrics_chart.gd")
## Wall seconds between asking the session for new rows.
const REFRESH_S := 1.0
## The animal table's column groups, in order. A column the session serves that no group
## names lands in "Other", so a new readout shows up without an edit here.
const GROUPS := [
	["Population", ["n", "n_rider", "n_pup", "n_juvenile", "n_adult", "native_n",
		"gestating_or_clutch_n", "breeding_eligible_n"]],
	["Condition", ["fullness_p10_rider", "fullness_p10_pup", "fullness_p10_juvenile", "fullness_p10_adult",
		"fullness_p50_rider", "fullness_p50_pup", "fullness_p50_juvenile", "fullness_p50_adult",
		"starving_n_rider", "starving_n_pup", "starving_n_juvenile", "starving_n_adult",
		"n_fullness_rider", "n_fullness_pup", "n_fullness_juvenile", "n_fullness_adult",
		"fullness_mean", "fullness_p10", "fullness_p50", "fullness_p90",
		"stomach_fill_mean", "stomach_fill_p50", "emergency_n", "eating_n", "asleep_n",
		"wound_mean", "wounded_n", "size_mean"]],
	["Activity", ["body_s", "awake_body_s", "moving_share", "feeding_share", "resting_share",
		"fleeing_share", "hunting_share", "climbing_share", "gliding_share", "flying_share"]],
	["Energy and budget", ["energy_per_body_mean", "energy_body_hours", "survivor_stock_change_per_body_hour",
		"cohort_energy_change", "intake_organic", "upkeep_organic", "motor_organic"]],
	["Movement", ["path_m_per_body", "displacement_m_per_body", "delivered_bl_per_s"]],
	["Stuck", ["stuck_n", "stuck_observed_n"]],
]
## Columns that are row keys, not values.
const KEYS := ["minute", "lineage", "species"]
## **Interim** series colours, by the lineage's id in `query("species")`: distinguishable,
## nothing more, until the session exposes species colours (it does not yet).
const INTERIM_SERIES_COLOURS := [
	Color(0.95, 0.62, 0.25), Color(0.4, 0.75, 0.95), Color(0.55, 0.85, 0.4),
	Color(0.95, 0.45, 0.55), Color(0.78, 0.6, 0.95), Color(0.95, 0.88, 0.35),
	Color(0.35, 0.85, 0.75), Color(0.95, 0.65, 0.85), Color(0.7, 0.7, 0.7),
	Color(0.6, 0.45, 0.3), Color(0.3, 0.55, 0.95), Color(0.85, 0.95, 0.65),
]

var shell
## The last `vitals` answer's columns: names, and name -> meaning.
var columns: Array = []
var plant_columns: Array = []
var docs := {}
var plant_docs := {}
## Rows by minute, and the minutes held, ascending.
var rows := {}
var plant_rows := {}
var minutes: Array = []
## Where the session's rows start (the recorder began there at load or founding).
var started = null
var view := "animals"

var _generation := -1
var _refresh_left := 0.0
var _follow := true
var _minute_at := -1
var _hidden_groups := {}
var _ids := {}  # lineage code name -> species id

var _status: Label
var _views := {}
var _pages := {}
var _minute_slider: HSlider
var _minute_label: Label
var _newest: CheckBox
var _groups_row: HBoxContainer
var _animal_grid: GridContainer
var _plant_grid: GridContainer
var _lineage_pick: OptionButton
var _column_pick: OptionButton
var _one: Chart
var _all: Chart
var _close: Button
var _listed: Array = []


func _init() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	visible = false
	var dim := Panel.new()
	dim.theme_type_variation = &"Dim"
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(dim)
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(margin)
	var panel := PanelContainer.new()
	margin.add_child(panel)
	var box := VBoxContainer.new()
	panel.add_child(box)

	var head := HBoxContainer.new()
	box.add_child(head)
	var title := Label.new()
	title.text = "Dev metrics"
	title.theme_type_variation = &"Title"
	head.add_child(title)
	var group := ButtonGroup.new()
	for v in [["animals", "Animals"], ["plants", "Plants"], ["charts", "Charts"]]:
		var b := Button.new()
		b.text = v[1]
		b.toggle_mode = true
		b.button_group = group
		b.custom_minimum_size = Vector2(120, 40)
		b.pressed.connect(show_view.bind(v[0]))
		head.add_child(b)
		_views[v[0]] = b
	var gap := Control.new()
	gap.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	head.add_child(gap)
	_close = Button.new()
	_close.text = "Back"
	_close.custom_minimum_size = Vector2(120, 40)
	_close.pressed.connect(back.emit)
	head.add_child(_close)

	_status = Label.new()
	_status.theme_type_variation = &"Small"
	box.add_child(_status)

	var minute_row := HBoxContainer.new()
	box.add_child(minute_row)
	var ml := Label.new()
	ml.text = "Minute"
	minute_row.add_child(ml)
	_minute_slider = HSlider.new()
	_minute_slider.custom_minimum_size = Vector2(320, 0)
	_minute_slider.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_minute_slider.step = 1
	_minute_slider.value_changed.connect(_on_minute_slider)
	minute_row.add_child(_minute_slider)
	_minute_label = Label.new()
	_minute_label.custom_minimum_size = Vector2(110, 0)
	minute_row.add_child(_minute_label)
	_newest = CheckBox.new()
	_newest.text = "Newest"
	_newest.button_pressed = true
	_newest.toggled.connect(func(on: bool):
		_follow = on
		_rebuild_tables())
	minute_row.add_child(_newest)

	# Animals.
	var animals := VBoxContainer.new()
	animals.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(animals)
	_pages["animals"] = animals
	_groups_row = HBoxContainer.new()
	animals.add_child(_groups_row)
	for g in GROUPS + [["Other", []]]:
		var c := CheckBox.new()
		c.text = g[0]
		c.button_pressed = true
		c.toggled.connect(func(on: bool):
			if on:
				_hidden_groups.erase(g[0])
			else:
				_hidden_groups[g[0]] = true
			_rebuild_tables())
		_groups_row.add_child(c)
	_animal_grid = _table(animals)

	# Plants.
	var plants := VBoxContainer.new()
	plants.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(plants)
	_pages["plants"] = plants
	_plant_grid = _table(plants)

	# Charts.
	var charts := VBoxContainer.new()
	charts.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(charts)
	_pages["charts"] = charts
	var picks := HBoxContainer.new()
	charts.add_child(picks)
	var l1 := Label.new()
	l1.text = "Lineage"
	picks.add_child(l1)
	_lineage_pick = OptionButton.new()
	_lineage_pick.custom_minimum_size = Vector2(220, 40)
	_lineage_pick.item_selected.connect(func(_i): _redraw_charts())
	picks.add_child(_lineage_pick)
	var l2 := Label.new()
	l2.text = "Column"
	picks.add_child(l2)
	_column_pick = OptionButton.new()
	_column_pick.custom_minimum_size = Vector2(380, 40)
	_column_pick.item_selected.connect(func(_i): _redraw_charts())
	picks.add_child(_column_pick)
	_one = Chart.new()
	charts.add_child(_one)
	_all = Chart.new()
	charts.add_child(_all)
	show_view("animals")


func _table(parent: Control) -> GridContainer:
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	parent.add_child(scroll)
	var grid := GridContainer.new()
	scroll.add_child(grid)
	return grid


# --- Opening ------------------------------------------------------------------------

## Show it over the world, with every row the session holds.
func open() -> void:
	visible = true
	_refresh(true)
	_views[view].grab_focus()


func show_view(v: String) -> void:
	view = v
	for k in _pages:
		_pages[k].visible = k == v
	_views[v].button_pressed = true
	_minute_slider.get_parent().visible = v != "charts"
	if v == "charts":
		_redraw_charts()
	else:
		_rebuild_tables()


func _process(delta: float) -> void:
	if not visible:
		return
	_refresh_left -= delta
	if _refresh_left <= 0.0:
		_refresh(false)


# --- Rows ---------------------------------------------------------------------------

func _session():
	return shell.session if shell != null else null


## Ask for the rows after the newest held (all of them on `full`, a new world or a new
## recorder start), file them, and redraw what shows.
func _refresh(full: bool) -> void:
	_refresh_left = REFRESH_S
	var s = _session()
	if s == null:
		return
	if _ids.is_empty():
		var table = s.query("species", null)
		if table is Dictionary:
			for a in table.get("animals", []):
				_ids[a["name"]] = int(a["id"])
	var generation: int = s.status().get("generation", 0)
	if generation != _generation:
		full = true
	var args = null if full or minutes.is_empty() else {"since": minutes[-1]}
	var q = s.query("vitals", args)
	if not (q is Dictionary) or q.has("error"):
		_status.text = "No vitals: %s" % (q.get("error", "") if q is Dictionary else "")
		return
	if not full and q.get("started_minute") != started:
		full = true
		q = s.query("vitals", null)
	_generation = generation
	if full:
		rows.clear()
		plant_rows.clear()
		minutes.clear()
		columns = q["animal_columns"].map(func(c): return c["name"])
		plant_columns = q["plant_columns"].map(func(c): return c["name"])
		docs.clear()
		for c in q["animal_columns"]:
			docs[c["name"]] = c["doc"]
		plant_docs.clear()
		for c in q["plant_columns"]:
			plant_docs[c["name"]] = c["doc"]
		_fill_column_pick()
	started = q.get("started_minute")
	var added := _file(q["animals"], rows) + _file(q["plants"], plant_rows)
	# Rows the session dropped off its ring's far end go here too.
	var from = q.get("from_minute")
	if from != null:
		for m in minutes.duplicate():
			if m < float(from) - 1e-9:
				rows.erase(m)
				plant_rows.erase(m)
				minutes.erase(m)
	_status.text = _status_text(q)
	if full or added > 0:
		_fill_lineage_pick()
		if view == "charts":
			_redraw_charts()
		else:
			_rebuild_tables()


func _status_text(q: Dictionary) -> String:
	if not q.get("enabled", true):
		return "The vitals recorder is off ([vitals] enabled = false)."
	if minutes.is_empty():
		return "No rows yet."
	return "%d minutes held, %s to %s (the recorder began at minute %s; rows are not saved, a load starts them again). Hover a column for its meaning." % [
		minutes.size(), Chart.label_of(minutes[0]), Chart.label_of(minutes[-1]),
		Chart.label_of(float(started)) if started != null else "?"]


## File `incoming` rows into `into` by minute; how many arrived.
func _file(incoming: Array, into: Dictionary) -> int:
	for r in incoming:
		var m := float(r[0])
		if not into.has(m):
			if not rows.has(m) and not plant_rows.has(m):
				minutes.append(m)
			into[m] = []
		into[m].append(r)
	minutes.sort()
	return incoming.size()


func shown_minute() -> float:
	if minutes.is_empty():
		return -1.0
	if _follow or _minute_at < 0 or _minute_at >= minutes.size():
		return minutes[-1]
	return minutes[_minute_at]


func _on_minute_slider(v: float) -> void:
	var i := int(v)
	if i == _minute_at:
		return
	_minute_at = i
	if _follow and i != minutes.size() - 1:
		_follow = false
		_newest.set_pressed_no_signal(false)
	_rebuild_tables()


## The lineage code names the animal table lists.
func listed_lineages() -> Array:
	return _listed


func lineage_label(name: String) -> String:
	return shell.species_label(name) if shell != null and shell.has_method("species_label") else name


func colour_of(name: String) -> Color:
	var i: int = _ids.get(name, absi(name.hash()))
	return INTERIM_SERIES_COLOURS[i % INTERIM_SERIES_COLOURS.size()]


# --- Tables -------------------------------------------------------------------------

## The animal columns to show, grouped: [[group, [names]], ...], without hidden groups.
func shown_groups() -> Array:
	var placed := {}
	var out := []
	for g in GROUPS:
		var names: Array = g[1].filter(func(n): return n in columns)
		for n in names:
			placed[n] = true
		if not _hidden_groups.has(g[0]) and not names.is_empty():
			out.append([g[0], names])
	var other: Array = columns.filter(func(n): return not placed.has(n) and n not in KEYS)
	if not _hidden_groups.has("Other") and not other.is_empty():
		out.append(["Other", other])
	return out


func _rebuild_tables() -> void:
	_minute_slider.max_value = maxi(minutes.size() - 1, 0)
	var shown := shown_minute()
	var i := minutes.find(shown)
	_minute_slider.set_value_no_signal(maxi(i, 0))
	_minute_at = i
	_minute_label.text = "" if shown < 0 else Chart.label_of(shown)
	if view == "animals":
		_build_animals(shown)
	elif view == "plants":
		_build_plants(shown)


func _clear(grid: GridContainer) -> void:
	for c in grid.get_children():
		grid.remove_child(c)
		c.queue_free()


func _cell(grid: GridContainer, text: String, tip := "") -> Label:
	var l := Label.new()
	l.text = text
	l.theme_type_variation = &"Small"
	if tip != "":
		l.tooltip_text = tip
		l.mouse_filter = Control.MOUSE_FILTER_PASS
	grid.add_child(l)
	return l


func _gap(grid: GridContainer) -> void:
	grid.add_child(Control.new())


static func value_text(v) -> String:
	if v == null:
		return "NA"
	if v is int:
		return str(v)
	if v is float:
		if is_equal_approx(v, roundf(v)) and absf(v) < 1e7:
			return str(int(v))
		return Chart.label_of(v)
	return str(v)


func _build_animals(minute: float) -> void:
	var grid := _animal_grid
	_clear(grid)
	_listed = []
	var groups := shown_groups()
	var names := []
	for g in groups:
		names += g[1]
	grid.columns = 1 + names.size()
	# Group names over each group's first column, then the column names.
	_cell(grid, "")
	for g in groups:
		_cell(grid, g[0])
		for k in g[1].size() - 1:
			_gap(grid)
	_cell(grid, "lineage", docs.get("lineage", ""))
	for n in names:
		_cell(grid, n, docs.get(n, ""))
	var at := columns.find("lineage")
	for r in rows.get(minute, []):
		var name: String = str(r[at])
		_listed.append(name)
		_cell(grid, lineage_label(name), name)
		for n in names:
			_cell(grid, value_text(r[columns.find(n)]))


func _build_plants(minute: float) -> void:
	var grid := _plant_grid
	_clear(grid)
	var names: Array = plant_columns.filter(func(n): return n not in KEYS)
	grid.columns = 1 + names.size()
	_cell(grid, "species", plant_docs.get("species", ""))
	for n in names:
		_cell(grid, n, plant_docs.get(n, ""))
	var at := plant_columns.find("species")
	for r in plant_rows.get(minute, []):
		_cell(grid, str(r[at]))
		for n in names:
			_cell(grid, value_text(r[plant_columns.find(n)]))


# --- Charts -------------------------------------------------------------------------

func _fill_column_pick() -> void:
	var keep := _column_pick.get_item_text(_column_pick.selected) if _column_pick.selected >= 0 else "n"
	_column_pick.clear()
	var i := 0
	for g in GROUPS + [["Other", []]]:
		for n in columns:
			if n in KEYS:
				continue
			var in_group: bool = n in g[1] if g[0] != "Other" else not GROUPS.any(func(x): return n in x[1])
			if not in_group:
				continue
			_column_pick.add_item(n)
			_column_pick.set_item_tooltip(i, docs.get(n, ""))
			if n == keep:
				_column_pick.select(i)
			i += 1
	if _column_pick.selected < 0 and _column_pick.item_count > 0:
		_column_pick.select(0)


func _fill_lineage_pick() -> void:
	var names := lineages()
	var keep = _lineage_pick.get_item_metadata(_lineage_pick.selected) if _lineage_pick.selected >= 0 else ""
	if names.size() == _lineage_pick.item_count:
		return
	_lineage_pick.clear()
	for k in names.size():
		_lineage_pick.add_item(lineage_label(names[k]))
		_lineage_pick.set_item_metadata(k, names[k])
		if names[k] == keep:
			_lineage_pick.select(k)


## Every lineage with a row in the ring, in the session's row order.
func lineages() -> Array:
	var out := []
	var at := columns.find("lineage")
	for m in minutes:
		for r in rows.get(m, []):
			if not out.has(r[at]):
				out.append(r[at])
	return out


func series_of(lineage: String, column: String) -> Array:
	var at := columns.find("lineage")
	var c := columns.find(column)
	var points := []
	if c < 0:
		return points
	for m in minutes:
		for r in rows.get(m, []):
			if r[at] == lineage:
				points.append([m, r[c]])
	return points


func _redraw_charts() -> void:
	if _column_pick.selected < 0 or _lineage_pick.selected < 0:
		_one.set_series([], "Pick a lineage and a column")
	else:
		var lineage: String = _lineage_pick.get_item_metadata(_lineage_pick.selected)
		var column := _column_pick.get_item_text(_column_pick.selected)
		_one.set_series([{"name": "%s: %s" % [lineage_label(lineage), column], "colour": colour_of(lineage),
			"points": series_of(lineage, column)}], docs.get(column, ""))
		_one.tooltip_text = docs.get(column, "")
	var all := []
	for name in lineages():
		all.append({"name": lineage_label(name), "colour": colour_of(name), "points": series_of(name, "n")})
	_all.set_series(all, "Population (n): living bodies of every lineage at each minute")
