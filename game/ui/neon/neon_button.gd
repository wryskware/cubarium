@tool
class_name NeonButton
extends BaseButton
## A button that holds other controls (a keycap and a label, a list row's dot, name and
## count, a chip) and draws its theme's state styles behind them, so the parts can use
## different fonts: Godot's Button has one font for every state.
##
## Theme items, looked up through theme_type_variation (MainButton, SecondaryButton, Tab,
## ListRow, Chip in neon_theme.tres, all based on Button):
## - styles normal, pressed, hover, hover_pressed, disabled: drawn behind the content, and
##   their content margins place it; focus is drawn on top while the button has focus.
## - constants min_height and min_width (px); press_drop (px the whole button moves down
##   while pressed); disabled_opacity (percent the button keeps while disabled).
##
## Child NeonLabels take their pressed_look while the button is pressed: held down, or
## toggled on (the selected tab, list row or chip).

## Draws the focus style without focus, for galleries that show every state at once.
@export var preview_focus := false:
	set(v):
		preview_focus = v
		queue_redraw()

var _applied := -1


func _ready() -> void:
	for c in get_children():
		_watch(c)
	child_entered_tree.connect(_watch)
	_apply_state()


func _watch(node: Node) -> void:
	if node is Control and not node.minimum_size_changed.is_connected(update_minimum_size):
		node.minimum_size_changed.connect(update_minimum_size)
	update_minimum_size()


func _notification(what: int) -> void:
	match what:
		NOTIFICATION_RESIZED, NOTIFICATION_THEME_CHANGED:
			_layout()


func _get_minimum_size() -> Vector2:
	var content := Vector2.ZERO
	for c in get_children():
		var k := c as Control
		if k != null and k.visible and not k.top_level:
			content = content.max(k.get_combined_minimum_size())
	# the current state's margins: a selected tab is narrower than the others
	var st := _style(get_draw_mode())
	var m := content + (st.get_minimum_size() if st != null else Vector2.ZERO)
	return m.max(Vector2(get_theme_constant("min_width"), get_theme_constant("min_height")))


func _draw() -> void:
	var mode := get_draw_mode()
	if mode != _applied:
		_apply_state.call_deferred()
	var r := Rect2(Vector2(0, _drop(mode)), size)
	var st := _style(mode)
	if st != null:
		draw_style_box(st, r)
	var hover_look := mode == DRAW_HOVER or mode == DRAW_HOVER_PRESSED
	if (has_focus() or preview_focus) and not hover_look and has_theme_stylebox("focus"):
		draw_style_box(get_theme_stylebox("focus"), r)


func _style(mode: int) -> StyleBox:
	var name: StringName = {DRAW_NORMAL: &"normal", DRAW_PRESSED: &"pressed", DRAW_HOVER: &"hover",
			DRAW_DISABLED: &"disabled", DRAW_HOVER_PRESSED: &"hover_pressed"}.get(mode, &"normal")
	if not has_theme_stylebox(name) and mode == DRAW_HOVER_PRESSED:
		name = &"pressed"
	if not has_theme_stylebox(name):
		name = &"normal"
	return get_theme_stylebox(name) if has_theme_stylebox(name) else null


func _drop(mode: int) -> int:
	return get_theme_constant("press_drop") if mode == DRAW_PRESSED or mode == DRAW_HOVER_PRESSED else 0


func _apply_state() -> void:
	if not is_inside_tree():
		return
	var mode := get_draw_mode()
	_applied = mode
	var keep := 100
	if disabled and has_theme_constant("disabled_opacity"):
		keep = get_theme_constant("disabled_opacity")
	modulate.a = keep / 100.0
	var on := is_pressed() and not disabled
	for l in find_children("*", "Label", true, false):
		if l.has_method("set_pressed_look"):
			l.set_pressed_look(on)
	update_minimum_size()
	_layout()


func _layout() -> void:
	var mode := get_draw_mode()
	var st := _style(mode)
	var r := Rect2(Vector2.ZERO, size)
	if st != null:
		r = Rect2(st.get_offset(), size - st.get_minimum_size())
	r.position.y += _drop(mode)
	for c in get_children():
		var k := c as Control
		if k != null and not k.top_level:
			k.position = r.position
			k.size = r.size
