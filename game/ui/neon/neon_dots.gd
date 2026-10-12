extends Node
## Autoload (project.godot: NeonDots). Gives every Label whose font asks for a drawn
## Gridlite field (Gridlite.field_spec) its two NeonDotField layers: the shadow layer
## behind the label and the text layer in front of it. A label is checked when it enters
## the tree and whenever its theme changes, so labels that switch looks (NeonLabel: a
## selected tab, a pressed button) or are made in code get theirs too. Labels in other
## fonts get nothing.

const _FIELDS := &"_neon_dot_fields"
const _WATCHED := &"_neon_dot_watched"


func _enter_tree() -> void:
	get_tree().node_added.connect(_on_node_added)


func _on_node_added(node: Node) -> void:
	var l := node as Label
	if l == null:
		return
	if not l.has_meta(_WATCHED):
		l.set_meta(_WATCHED, true)
		l.theme_changed.connect(_check.bind(l))
	# a node entering the tree can't take children yet
	_check.call_deferred(l)


func _check(l) -> void:
	if not is_instance_valid(l) or not l.is_inside_tree() or l.has_meta(_FIELDS):
		return
	if Gridlite.field_spec(l.get_theme_font("font")).is_empty():
		return
	l.set_meta(_FIELDS, true)
	for layer in [NeonDotField.Layer.SHADOW, NeonDotField.Layer.TEXT]:
		var f := NeonDotField.new()
		f.name = "DotShadow" if layer == NeonDotField.Layer.SHADOW else "DotField"
		f.layer = layer
		f.show_behind_parent = layer == NeonDotField.Layer.SHADOW
		f.mouse_filter = Control.MOUSE_FILTER_IGNORE
		f.focus_mode = Control.FOCUS_NONE
		f.follow_label = true
		l.add_child(f, false, Node.INTERNAL_MODE_BACK)
