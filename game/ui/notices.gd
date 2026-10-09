extends VBoxContainer
## Plain notices in the lower left: what a save or a load did, in words. Each stays a few
## seconds, then fades out (never pops).
##
## The look is notices.tscn and one notice.tscn per message (a NoticeBox with SmallText,
## WarningText for a warning), laid out in the editor from ui/neon/. Without a shell
## (opened in the editor, or by itself) the scene's two sample notices stay.

const NOTICE := preload("res://ui/notice.tscn")
const SHOWN_S := 6.0
const FADE_S := 0.8

## The shell (main.gd); null in the editor, which keeps the samples.
var shell: Node
## Every notice said, oldest first (tests read it).
var said: Array[String] = []


func _ready() -> void:
	if shell == null:
		return
	for c in get_children():
		remove_child(c)
		c.queue_free()


func add(text: String, warn := false) -> void:
	said.append(text)
	var panel: Control = NOTICE.instantiate()
	var label: Label = panel.get_node("Margin/Text")
	label.text = text
	if text.length() > 70:  # long ones wrap at a fixed width; short ones fit their text
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		label.custom_minimum_size = Vector2(660, 0)
	if warn:
		label.theme_type_variation = &"WarningText"
	add_child(panel)
	panel.modulate.a = 0.0
	var t := panel.create_tween()
	t.tween_property(panel, "modulate:a", 1.0, 0.2)
	t.tween_interval(SHOWN_S * (2.0 if warn else 1.0))
	t.tween_property(panel, "modulate:a", 0.0, FADE_S)
	t.tween_callback(panel.queue_free)
