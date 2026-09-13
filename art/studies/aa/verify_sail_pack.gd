extends SceneTree
## Check complete source regeneration; install only the verified creature atlas.
func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 3 or (args.size() == 4 and args[3] == "--body-only"), "ORIGINAL_PACK CANDIDATE_PACK REPORT_PATH [--body-only]")
	var body_only := args.size() == 4
	var a := Image.load_from_file(args[0].path_join("creatures.png"))
	var b := Image.load_from_file(args[1].path_join("creatures.png"))
	assert(a.get_size() == b.get_size())
	var changed: Array = []
	for row in range(16):
		var equal := a.get_region(Rect2i(0,row*16,256,16)).get_data() == b.get_region(Rect2i(0,row*16,256,16)).get_data()
		if not equal: changed.append(row)
		if row < 4 or row > 7: assert(equal, "Unrelated creature row changed")
		if body_only and row != 5: assert(equal, "Body-only changed non-move row")
	if body_only: assert(changed == [5], "Body-only must change exactly sail move")
	var exact: Array = []
	for file in DirAccess.get_files_at(args[0]):
		if file == "creatures.png": continue
		assert(FileAccess.get_file_as_bytes(args[0].path_join(file)) == FileAccess.get_file_as_bytes(args[1].path_join(file)), "Unrelated pack file changed: " + file)
		exact.append(file)
	var report := FileAccess.open(args[2], FileAccess.WRITE)
	report.store_string(JSON.stringify({"passed":true,"body_only":body_only,"changed_creature_rows":changed,"all_other_rows_byte_exact":true,"other_files_byte_exact":exact},"  "))
	print(report.get_path())
	quit()
