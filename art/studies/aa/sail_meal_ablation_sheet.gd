extends SceneTree
## Reuse exact same selected meal frame/crop evidence, no new visual selection.
func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 3, "BODY_ONLY_MEAL COMBINED_MEAL NEW_OUTPUT_PNG")
	var a: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(args[0].path_join("meal-contact.json")))
	var b: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(args[1].path_join("meal-contact.json")))
	assert(a.selected == b.selected, "Meal selections/positions must be identical")
	var body := Image.load_from_file(args[0].path_join("meal-contact.png"))
	var combined := Image.load_from_file(args[1].path_join("meal-contact.png"))
	assert(body.get_region(Rect2i(0,0,128,1024)).get_data() == combined.get_region(Rect2i(0,0,128,1024)).get_data(), "Original crop stream differs")
	var sheet := Image.create(384,1024,false,Image.FORMAT_RGB8)
	sheet.blit_rect(body,Rect2i(0,0,256,1024),Vector2i.ZERO)
	sheet.blit_rect(combined,Rect2i(128,0,128,1024),Vector2i(256,0))
	assert(sheet.save_png(args[2]) == OK)
	quit()
