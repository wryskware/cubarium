@tool
extends EditorPlugin

func _enter_tree() -> void:
	add_tool_menu_item("Bake Cubarium art", bake)

func _exit_tree() -> void:
	remove_tool_menu_item("Bake Cubarium art")

func bake() -> void:
	get_editor_interface().save_all_scenes()
	var output: Array = []
	var code := OS.execute(OS.get_executable_path(), ["--headless", "--path", ProjectSettings.globalize_path("res://"), "--script", "bake.gd"], output, true)
	for line in output: print(line)
	if code != 0:
		push_error("Cubarium art bake failed; see Output for details.")
	else:
		print("Cubarium art baked. Restart the art-study preview to load the new atlas.")
