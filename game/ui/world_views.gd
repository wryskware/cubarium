extends RefCounted
## Where the shell gets a `WorldView`: package B's scene when it exists
## (`res://world/world_view.tscn`), else the shell's stub with the same API
## (`res://ui/dev/world_view_stub.gd`). The contract is in
## `design/handoffs/godot-direction-2026-10-07.md`, "B and C in parallel".

const REAL := "res://world/world_view.tscn"
const STUB := preload("res://ui/dev/world_view_stub.gd")


static func is_real() -> bool:
	return ResourceLoader.exists(REAL)


## A new WorldView, not yet in the tree.
static func make() -> Node:
	if is_real():
		var scene := load(REAL) as PackedScene
		if scene != null:
			return scene.instantiate()
	return STUB.new()


## A WorldView drawing one static view inside its own SubViewport (its own 3D world, so
## candidates never share a scene with each other or the live world), wrapped in a
## container that stretches it and passes pointer input through.
static func make_framed(view: Dictionary, input := false) -> SubViewportContainer:
	var box := SubViewportContainer.new()
	box.stretch = true
	box.mouse_filter = Control.MOUSE_FILTER_STOP if input else Control.MOUSE_FILTER_IGNORE
	var port := SubViewport.new()
	port.own_world_3d = true
	port.handle_input_locally = true
	port.render_target_update_mode = SubViewport.UPDATE_WHEN_VISIBLE
	box.add_child(port)
	var wv := make()
	port.add_child(wv)
	if wv is Control:
		(wv as Control).set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	wv.set("input_enabled", input)
	wv.show_view(view)
	return box


## The WorldView inside a container from `make_framed`.
static func framed_view(box: SubViewportContainer) -> Node:
	if box == null or box.get_child_count() == 0:
		return null
	var port := box.get_child(0)
	return port.get_child(0) if port.get_child_count() > 0 else null
