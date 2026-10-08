extends Node
## The bare shell: make a world from the desktop terrarium's parameters, run it, and
## print what it is doing every few seconds. Nothing is drawn yet (package B draws,
## package C builds the menus).

## The parameter set: a config file's simulation half, from the repository.
const CONFIG := "../config/desktop/terrarium.toml"
const REPORT_EVERY_S := 5.0

var session: CubariumSession
var since := 0.0
var promoted := false


func _ready() -> void:
	session = CubariumSession.new()
	var config := ProjectSettings.globalize_path("res://").path_join(CONFIG).simplify_path()
	var spec := {"config": config}
	# `godot --path game -- --seed N` founds that seed; without one a seed is drawn (and a
	# refused habitat redrawn, which can take a few tries).
	var args := OS.get_cmdline_user_args()
	var at := args.find("--seed")
	if at >= 0 and at + 1 < args.size():
		spec["seed"] = int(args[at + 1])
	var refused := session.generate_candidates(spec, 1)
	if refused != "":
		push_error("cubarium: " + refused)
		return
	print("cubarium: founding a world from ", config)


func _process(delta: float) -> void:
	if session == null:
		return
	if not promoted:
		var c: Array = session.candidates()
		if c.size() > 0 and c[0]["state"] == "ready":
			var refused := session.promote(0)
			print("cubarium: ", refused if refused != "" else "running " + str(c[0]["label"]))
			promoted = true
		elif c.size() > 0 and c[0]["state"] == "failed":
			push_error("cubarium: " + str(c[0]["error"]))
			session = null
		return
	since += delta
	if since < REPORT_EVERY_S:
		return
	since = 0.0
	var summary = session.query("summary", null)
	var pops = session.query("populations", null)
	print("cubarium: ", JSON.stringify(summary))
	var alive := []
	for a in pops.get("animals", []):
		alive.append("%s %d" % [a["label"], a["alive"]])
	print("cubarium: tick ", summary.get("tick", 0), " — ", ", ".join(alive))
