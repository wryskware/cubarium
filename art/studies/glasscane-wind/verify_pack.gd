extends SceneTree
## ORIGINAL CANDIDATE NEW_REPORT.json — the candidate pack differs from the original in
## exactly the cleared rows and nothing else: `tall.png` atlas rows y = 64 and 79 (the
## glasscane trunk row, tile rows 0 and 15) and y = 95 (the glasscane crown row, tile row
## 15), every changed pixel painted in the original and transparent in the candidate, and
## within the trunk's four columns (tile columns 6–9). Every other file, `pack.json`
## included, is byte-identical.
const TRUNK_ROW := 4
const CROWN_ROW := 5
const TILE := 16

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 3, "ORIGINAL CANDIDATE NEW_REPORT")
	assert(not FileAccess.file_exists(args[2]), "report must be new")
	var a := Image.load_from_file(args[0].path_join("tall.png"))
	var b := Image.load_from_file(args[1].path_join("tall.png"))
	assert(a != null and b != null)
	assert(a.get_size() == b.get_size())
	var allowed := [TRUNK_ROW * TILE + 0, TRUNK_ROW * TILE + 15, CROWN_ROW * TILE + 15]
	var changed := {}
	for y in allowed:
		changed[y] = 0
	var total := 0
	for y in range(a.get_height()):
		for x in range(a.get_width()):
			var pa := a.get_pixel(x, y)
			var pb := b.get_pixel(x, y)
			if pa == pb:
				continue
			assert(y in allowed, "changed pixel outside the cleared rows at %d,%d" % [x, y])
			assert(pa.a > 0.0, "an unpainted original pixel changed at %d,%d" % [x, y])
			assert(pb.a == 0.0 and pb.r == 0.0 and pb.g == 0.0 and pb.b == 0.0, "candidate pixel not transparent at %d,%d" % [x, y])
			assert(x % TILE >= 6 and x % TILE <= 9, "changed pixel outside the trunk columns at %d,%d" % [x, y])
			changed[y] += 1
			total += 1
	# The trunk's two end rows are the full 4-wide pattern in every one of 24 frames; the
	# crown's row 15 is the same 4-wide pattern.
	for y in allowed:
		assert(changed[y] == 4 * 24, "row %d: %d pixels changed, expected 96" % [y, changed[y]])
	assert(total == 3 * 4 * 24)
	# Every candidate row that stays must still be exactly the original.
	var exact: Array = []
	for file in DirAccess.get_files_at(args[0]):
		if file == "tall.png":
			continue
		assert(FileAccess.get_file_as_bytes(args[0].path_join(file)) == FileAccess.get_file_as_bytes(args[1].path_join(file)), file + " differs")
		exact.append(file)
	assert("pack.json" in exact and "plants.png" in exact and "creatures.png" in exact and "ground.png" in exact and "habitat.png" in exact)
	var report := {
		"passed": true,
		"changed_pixels": total,
		"changed_atlas_rows": {"trunk_row0": changed[allowed[0]], "trunk_row15": changed[allowed[1]], "crown_row15": changed[allowed[2]]},
		"other_files_exact": exact,
	}
	FileAccess.open(args[2], FileAccess.WRITE).store_string(JSON.stringify(report, "  ") + "\n")
	print("verified: %d pixels cleared, %s exact" % [total, ", ".join(exact)])
	quit()
