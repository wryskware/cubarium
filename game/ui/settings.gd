extends Control
## The Settings page: how the animals move. **Movement**: Smooth (they glide between
## ticks) or Stepped (they hop a voxel at a time and turn in steps, as the old stamped
## animals did). **Subpixel** placement, for Smooth only: on, they glide between pixels;
## off, they move a whole pixel at a time. A change applies to the running world at once
## (`changed`); the shell keeps it for this viewer. Turn counts and blends stay in the
## presets and the config.
##
## The look is settings.tscn, laid out in the editor from ui/neon/ (round 4's Settings
## view): Movement is a two-stop NeonDotSlider, Subpixel a NeonCheckbox, greyed under
## Stepped. This script only wires them. It is the tabbed page's Settings tab
## (ui/tabbed_page/), which holds the frame, tabs and hints around it. Without a shell
## (the editor, or a picture of the scene) it keeps the scene's sample values; under the
## shell it starts hidden and the tabbed page shows it.

signal changed(movement: String, subpixel: bool)

## The slider's stops, left to right.
const MOVEMENTS := ["smooth", "stepped"]

## The shell (main.gd); null in the editor or a picture, which keeps the sample values.
var shell: Node
var movement := "smooth"
var subpixel := true

@onready var _movement: NeonDotSlider = %Movement
@onready var _subpixel: NeonCheckbox = %Subpixel


func _ready() -> void:
	_movement.changed.connect(_on_movement)
	_subpixel.toggled.connect(_on_subpixel)
	if shell == null:
		# By itself: the sample page stays as laid out.
		movement = MOVEMENTS[_movement.index]
		subpixel = _subpixel.button_pressed
		_sync()
		return
	visible = false
	_sync()


func _on_movement(i: int) -> void:
	var value: String = MOVEMENTS[i]
	if movement == value:
		return
	movement = value
	_sync()
	changed.emit(movement, subpixel)


func _on_subpixel(on: bool) -> void:
	subpixel = on
	_sync()
	changed.emit(movement, subpixel)


## Show it with the settings in force.
func open(p_movement: String, p_subpixel: bool) -> void:
	set_values(p_movement, p_subpixel)
	visible = true
	focus_first()


## Show these settings without reporting a change.
func set_values(p_movement: String, p_subpixel: bool) -> void:
	movement = "stepped" if p_movement == "stepped" else "smooth"
	subpixel = p_subpixel
	_sync()


func focus_first() -> void:
	_movement.grab_focus()


## Movement: Smooth or Stepped, as a viewer would move the slider.
func choose_movement(value: String) -> void:
	_movement.choose(MOVEMENTS.find("stepped" if value == "stepped" else "smooth"))


## Subpixel on or off, as a viewer would tick it (nothing under Stepped, where it is off).
func choose_subpixel(on: bool) -> void:
	if not _subpixel.disabled:
		_subpixel.button_pressed = on


func subpixel_enabled() -> bool:
	return not _subpixel.disabled


func _sync() -> void:
	_movement.index = MOVEMENTS.find(movement)
	_subpixel.set_pressed_no_signal(subpixel)
	_subpixel.disabled = movement == "stepped"
	_subpixel.queue_redraw()
