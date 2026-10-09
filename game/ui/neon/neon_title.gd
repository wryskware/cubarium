@tool
class_name NeonTitle
extends Label
## A title in the title style (theme type variation TitleLabel) with Gridlite's unlit dot
## field running one space past each end: the label shows `title` with `pad` on both
## sides. Wrysk, 2026-10-09, on a space each side of the title: "yes space. definitely."
## Set `title`, not `text`; `text` follows it.

## The words, without the padding.
@export var title := "":
	set(v):
		title = v
		_update()
## Added to both ends: one space by default.
@export var pad := " ":
	set(v):
		pad = v
		_update()


func _ready() -> void:
	if title == "" and text.strip_edges() != "":
		title = text.strip_edges()
	_update()


func _update() -> void:
	text = pad + title + pad
