extends "res://studies/aa/bake_sail.gd"
## Two authored-motion candidates, always the original point compositor.
## Changes loaded animation resources only; never writes production source or pack.
const VARIANTS = ["brace", "settle"]

func change_keys(clip: Animation, path: String, times: Array, values: Array) -> void:
	var track := clip.find_track(NodePath(path), Animation.TYPE_VALUE)
	if track < 0: fail("Missing track " + path); return
	while clip.track_get_key_count(track) > 0: clip.track_remove_key(track, 0)
	for k in range(times.size()): clip.track_insert_key(track, times[k], values[k])

func authored(rig: Node2D, variant: String) -> void:
	var player := rig.get_node("AnimationPlayer") as AnimationPlayer
	var library := player.get_animation_library("").duplicate(true) as AnimationLibrary
	player.remove_animation_library("")
	player.add_animation_library("", library)
	for side in ["LeftFin", "RightFin"]:
		var sign := 1.0 if side == "LeftFin" else -1.0
		var rest: Array = [0.0, 0.0, 0.2, 0.0, 0.0] if variant == "brace" else [0.12, 0.12, 0.0, 0.12, 0.12]
		var feed: Array = [0.2, 0.2, 0.2, 0.2, 0.2] if variant == "brace" else [0.2, 0.2, 0.0, 0.2, 0.2]
		change_keys(player.get_animation("rest"), side + ":rotation", [0.0, 2.5, 3.0, 3.5, 4.0], rest.map(func(a):return a * sign))
		change_keys(player.get_animation("feed"), side + ":rotation", [0.0, 0.25, 0.5, 0.75, 2.0], feed.map(func(a):return a * sign))

func study() -> void:
	var output := ""
	for argument in OS.get_cmdline_user_args():
		if argument.begins_with("--out="): output = argument.trim_prefix("--out=")
	if output.is_empty() or DirAccess.dir_exists_absolute(output) or DirAccess.make_dir_recursive_absolute(output) != OK:
		push_error("Sail calm requires a NEW --out directory"); quit(1); return
	var pack_path := ProjectSettings.globalize_path("res://../assets/atelier")
	var atlas := Image.load_from_file(pack_path.path_join("creatures.png")); atlas.convert(Image.FORMAT_RGBA8)
	var metadata: Array = []
	for variant in ["original"] + VARIANTS:
		var dir := output.path_join(variant); DirAccess.make_dir_recursive_absolute(dir)
		var pack_dir := dir.path_join("pack"); DirAccess.make_dir_recursive_absolute(pack_dir)
		for name in DirAccess.get_files_at(pack_path):
			if DirAccess.copy_absolute(pack_path.path_join(name), pack_dir.path_join(name)) != OK: fail("copy " + name)
		var rig := (load("res://creatures/sail.tscn") as PackedScene).instantiate() as Node2D
		rig.set_meta("sail_fin_coverage4", false);root.add_child(rig)
		if variant != "original": authored(rig, variant)
		var scene := PackedScene.new();scene.pack(rig);ResourceSaver.save(scene, dir.path_join("sail.tscn"))
		var player := rig.get_node("AnimationPlayer") as AnimationPlayer
		player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
		var sprites: Array[Sprite2D] = [];collect(rig,sprites)
		if sprites.size()!=4: fail("sail layer count")
		for i in range(mini(4,sprites.size())):
			if str(rig.get_path_to(sprites[i]))!=PATHS[i] or sprites[i].texture.resource_path!=SOURCES[i]: fail("sail layer contract")
		var combined := atlas.duplicate() as Image
		var cases: Array = []
		for mode_index in range(MODES.size()):
			var mode: String = MODES[mode_index];var clip := player.get_animation(mode)
			var strip := Image.create(TILE*FRAMES,TILE,false,Image.FORMAT_RGBA8)
			var body := Image.create(TILE*FRAMES,TILE,false,Image.FORMAT_RGBA8)
			for frame in range(FRAMES):
				select_pose(player,mode,float(frame)/(FRAMES-1 if mode=="bud" else FRAMES)*clip.length)
				var image := raster_point(rig);strip.blit_rect(image,Rect2i(0,0,TILE,TILE),Vector2i(frame*TILE,0))
				sprites[0].visible=false;sprites[1].visible=false
				body.blit_rect(raster_point(rig),Rect2i(0,0,TILE,TILE),Vector2i(frame*TILE,0))
				sprites[0].visible=true;sprites[1].visible=true
			var old := atlas.get_region(Rect2i(0,(4+mode_index)*TILE,TILE*FRAMES,TILE))
			if variant=="original" and strip.get_data()!=old.get_data():fail("original atlas mismatch " + mode)
			if mode in ["move","bud"] and strip.get_data()!=old.get_data():fail("protected mode changed " + mode)
			if variant!="original":
				var old_body := Image.load_from_file(output.path_join("original/sail-"+mode+"-body.png"));old_body.convert(Image.FORMAT_RGBA8)
				if body.get_data()!=old_body.get_data():fail("body/bud layer changed " + mode)
			select_pose(player,mode,clip.length)
			if raster_point(rig).get_data()!=strip.get_region(Rect2i((FRAMES-1 if mode=="bud" else 0)*TILE,0,TILE,TILE)).get_data():fail("loop endpoint " + mode)
			strip.save_png(dir.path_join("sail-"+mode+".png"));body.save_png(dir.path_join("sail-"+mode+"-body.png"))
			combined.blit_rect(strip,Rect2i(0,0,TILE*FRAMES,TILE),Vector2i(0,(4+mode_index)*TILE))
			cases.append({"name":"sail-"+mode,"clip":mode,"seconds":clip.length,"loop":mode!="bud","frames":FRAMES})
		combined.save_png(pack_dir.path_join("creatures.png"))
		metadata.append({"variant":variant,"cases":cases,"point_sampling":true,"body_bud_exact":true,"move_bud_exact":true,"authored_endpoints_exact":true})
		rig.queue_free()
	var file := FileAccess.open(output.path_join("cases.json"),FileAccess.WRITE)
	file.store_string(JSON.stringify({"passed":not failed,"variants":metadata},"  "))
	print(JSON.stringify({"passed":not failed,"output":output}));quit(1 if failed else 0)
