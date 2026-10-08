extends VBoxContainer
## Plain notices in the lower left: what a save or a load did, in words. Each stays a few
## seconds, then fades out (never pops).

const SHOWN_S := 6.0
const FADE_S := 0.8

## Every notice said, oldest first (tests read it).
var said: Array[String] = []


func _init() -> void:
	anchor_left = 0.0
	anchor_right = 0.0
	anchor_top = 1.0
	anchor_bottom = 1.0
	offset_left = 16
	offset_right = 720
	offset_bottom = -16
	grow_vertical = Control.GROW_DIRECTION_BEGIN
	alignment = BoxContainer.ALIGNMENT_END
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_theme_constant_override("separation", 6)


func add(text: String, warn := false) -> void:
	said.append(text)
	var panel := PanelContainer.new()
	panel.theme_type_variation = &"Tip"
	panel.mouse_filter = Control.MOUSE_FILTER_IGNORE
	panel.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	var label := Label.new()
	label.text = text
	if text.length() > 70:  # long ones wrap at a fixed width; short ones fit their text
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		label.custom_minimum_size = Vector2(660, 0)
	if warn:
		label.theme_type_variation = &"Warn"
	panel.add_child(label)
	add_child(panel)
	panel.modulate.a = 0.0
	var t := panel.create_tween()
	t.tween_property(panel, "modulate:a", 1.0, 0.2)
	t.tween_interval(SHOWN_S * (2.0 if warn else 1.0))
	t.tween_property(panel, "modulate:a", 0.0, FADE_S)
	t.tween_callback(panel.queue_free)
