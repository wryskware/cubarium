extends Node2D

const MODES = ["rest", "move", "feed", "bud"]
var rigs: Array[Node] = []
var status: Label

func label_at(words: String, point: Vector2, size: int, color := Color("a7adc5")) -> Label:
	var label := Label.new()
	label.text = words
	label.position = point
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	add_child(label)
	return label

func _ready() -> void:
	rigs = [$Lantern, $Sail, $Mossback]
	label_at("CUBARIUM  /  CREATURE ATELIER", Vector2(36, 28), 24, Color("d6dfd1"))
	label_at("Three shape studies · editable sprite parts and animation timelines", Vector2(36, 66), 16)
	for index in range(3):
		label_at(["LANTERN", "SAIL", "MOSSBACK"][index], Vector2(100 + index * 285, 350), 20, Color("d6dfd1"))
		label_at(["A deliberate little walker", "A quiet, folding silhouette", "A compact, uneven crown"][index], Vector2(65 + index * 285, 382), 14)
	for index in range(MODES.size()):
		var button := Button.new()
		button.text = str(index + 1) + " · " + MODES[index].capitalize()
		button.position = Vector2(36 + index * 155, 445)
		button.size = Vector2(142, 36)
		button.pressed.connect(set_mode.bind(MODES[index]))
		add_child(button)
	status = label_at("", Vector2(36, 498), 14)
	label_at("HABITAT", Vector2(686, 414), 12)
	for index in range(3):
		var plant := Sprite2D.new()
		plant.texture = load("res://habitat/" + ["rosette", "fern", "lichen"][index] + ".svg")
		plant.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		plant.position = Vector2(702 + index * 65, 465)
		plant.scale = Vector2(4, 4)
		add_child(plant)
	set_mode("rest")

func set_mode(mode: String) -> void:
	for rig in rigs:
		var player: AnimationPlayer = rig.get_node("AnimationPlayer")
		player.play("RESET")
		player.advance(0)
		player.play(mode)
		player.advance(0)
	status.text = "Pose: " + mode + "    ·    Open a creature scene to edit its parts and timeline."

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		var index: int = event.keycode - KEY_1
		if index >= 0 and index < MODES.size():
			set_mode(MODES[index])
