extends SceneTree
## A still thumbnail lets its renderer go once a frame says `settled` (nothing more is
## coming: lit, the fast tier alone, or the wait for its lighting given up), and not before.
## A candidate opened large keeps its renderer. A stub renderer hands the frames over.
## Run headless: scripts/hidden.sh godot --headless --path game --script res://tests/thumbnail_release_test.gd

const View := preload("res://world/world_view.gd")
var failures := 0


class RendererStub:
	extends RefCounted
	var frames: Array = []
	var released := 0
	func take_frame():
		return frames.pop_front() if not frames.is_empty() else null
	func release() -> void:
		released += 1
	func set_camera(_zoom: float, _center: Vector2, _screen: Vector2i) -> void:
		pass
	func detach() -> void:
		pass


func check(ok: bool, message: String) -> void:
	print(("PASS " if ok else "FAIL ") + message)
	if not ok:
		failures += 1


func _initialize() -> void:
	run.call_deferred()


## One 4x2 frame of a still world: its lighting and whether it is the last.
func frame(lit: bool, settled = null) -> Dictionary:
	var f := {
		"rgba": PackedByteArray(), "width": 4, "height": 2, "view_x": 0, "generation": 1, "tick": 0,
		"lighting_ready": lit,
	}
	f["rgba"].resize(4 * 2 * 4)
	if settled != null:
		f["settled"] = settled
	return f


## A view showing a still world (`thumbnail`: input off), drawn by `stub`.
func still_view(stub: RendererStub, thumbnail: bool) -> Control:
	var view: Control = View.new()
	view.size = Vector2(64, 32)
	root.add_child(view)
	view.input_enabled = not thumbnail
	view._static = true
	view.renderer = stub
	return view


func run() -> void:
	# A thumbnail whose wait for its lighting gave up: unlit, settled.
	var stub := RendererStub.new()
	var view := still_view(stub, true)
	stub.frames = [frame(false, false)]
	view._take_frame()
	check(stub.released == 0, "a first picture still to be lit keeps its renderer")
	stub.frames = [frame(false, true)]
	view._take_frame()
	check(view.settled and not view.lighting_ready, "the view says settled without lighting")
	check(stub.released == 1, "a thumbnail lets its renderer go once settled, lit or not")
	view.queue_free()

	# A thumbnail refined to the end: its sky shading easing in, then lit and eased.
	stub = RendererStub.new()
	view = still_view(stub, true)
	stub.frames = [frame(false, false)]
	view._take_frame()
	check(stub.released == 0, "a picture still easing its shading in keeps the renderer")
	stub.frames = [frame(true, true)]
	view._take_frame()
	check(stub.released == 1, "the lit, settled frame lets it go")
	view.queue_free()

	# A candidate opened large keeps its renderer, settled or not.
	stub = RendererStub.new()
	view = still_view(stub, false)
	stub.frames = [frame(false, true)]
	view._take_frame()
	check(stub.released == 0, "a candidate opened large keeps its renderer")
	view.queue_free()

	print("THUMBNAIL RELEASE TEST: %d failures" % failures)
	quit(0 if failures == 0 else 1)
