@tool
extends NeonButton
## One row of the field guide's species list (species_row.tscn): the swatch, the name and
## the live count. The look is the scene's (ListRow, ListText, ChromeLabel in
## ui/neon/neon_theme.tres): edit species_row.tscn and every row follows. The guide sets
## these three per species; `species` is the sim's name for it (not shown).

@export var title := "Lanternjaw":
	set(v):
		title = v
		_sync()
@export var swatch := Color("#9168A1"):
	set(v):
		swatch = v
		_sync()
@export var count := "7":
	set(v):
		count = v
		_sync()

var species := ""


func _ready() -> void:
	super()
	_sync()


func _sync() -> void:
	if not is_node_ready():
		return
	$Content/Swatch.color = swatch
	$Content/Name.text = title
	$Content/Count.text = count
