extends SceneTree
## How long candidates take to make, terrain only (`empty`) against founded:
##   godot --headless --path game --script res://tests/found_timing.gd -- \
##       --config PATH [--count N] [--empty] [--seed S]
## Prints one line: `TIMING config=… count=N empty=… ms=… states=…` and exits. Wall time
## from the request to the last candidate leaving `generating`; a timing, not a check.

const TIMEOUT_S := 3000.0

var session
var started := 0
var line := ""


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	var config := ""
	var count := 1
	var empty := false
	var seed := 7
	for i in args.size():
		match args[i]:
			"--config": config = args[i + 1]
			"--count": count = int(args[i + 1])
			"--empty": empty = true
			"--seed": seed = int(args[i + 1])
	session = ClassDB.instantiate("CubariumSession")
	line = "TIMING config=%s count=%d empty=%s" % [config.get_file(), count, empty]
	started = Time.get_ticks_msec()
	var spec := {"config": config, "seed": seed, "empty": empty}
	var refused: String = session.generate_candidates(spec, count)
	if refused != "":
		print(line, " refused=", refused)
		quit(1)


func _process(_delta: float) -> bool:
	var c: Array = session.candidates()
	var waiting := c.filter(func(x): return x["state"] == "generating")
	var ms := Time.get_ticks_msec() - started
	if waiting.is_empty() or ms > TIMEOUT_S * 1000.0:
		var states := ",".join(c.map(func(x): return x["state"]))
		print(line, " ms=", ms, " states=", states)
		quit(0)
		return true
	return false
