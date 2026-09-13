extends SceneTree
func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 3, "ORIGINAL CLEAR_ENDS NEW_REPORT")
	assert(not FileAccess.file_exists(args[2]))
	var a := Image.load_from_file(args[0].path_join("tall.png"))
	var b := Image.load_from_file(args[1].path_join("tall.png"))
	assert(a.get_size() == b.get_size())
	var changed := 0
	for y in range(a.get_height()):
		for x in range(a.get_width()):
			if a.get_pixel(x,y) == b.get_pixel(x,y): continue
			assert(y == 96 or y == 111, "Changed non-endpoint row")
			assert(b.get_pixel(x,y).a == 0.0)
			changed += 1
	assert(changed > 0)
	var exact: Array = []
	for file in DirAccess.get_files_at(args[0]):
		if file == "tall.png": continue
		assert(FileAccess.get_file_as_bytes(args[0].path_join(file)) == FileAccess.get_file_as_bytes(args[1].path_join(file)),file)
		exact.append(file)
	FileAccess.open(args[2],FileAccess.WRITE).store_string(JSON.stringify({"passed":true,"changed_pixels":changed,"changed_atlas_rows":[96,111],"other_files_exact":exact},"  "))
	quit()
