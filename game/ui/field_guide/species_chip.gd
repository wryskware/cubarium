@tool
extends NeonButton
## One chip of a field guide entry's Eats or Eaten by row (species_chip.tscn): a swatch and
## a name. The look is the scene's (Chip, ChipText in ui/neon/neon_theme.tres): edit
## species_chip.tscn and every chip follows. A species chip opens that species' page; a
## food-class chip (`species` empty) is a label only.

@export var title := "Carrion":
	set(v):
		title = v
		_sync()
@export var swatch := Color("#A595D8"):
	set(v):
		swatch = v
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
