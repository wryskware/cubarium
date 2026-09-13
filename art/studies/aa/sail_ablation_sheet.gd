extends "res://bake.gd"
## Three fixed packs; native source strips and exact known body-mask fin partition.
func _initialize() -> void:
	call_deferred("study")

func study() -> void:
	var args := OS.get_cmdline_user_args()
	assert(args.size() == 4, "ORIGINAL_PACK BODY_ONLY_PACK COMBINED_PACK NEW_OUTPUT")
	assert(not DirAccess.dir_exists_absolute(args[3]))
	assert(DirAccess.make_dir_recursive_absolute(args[3]) == OK)
	var atlases: Array[Image] = []
	for path in args.slice(0,3): atlases.append(Image.load_from_file(path.path_join("creatures.png")))
	var rig := (load("res://creatures/sail.tscn") as PackedScene).instantiate() as Node2D
	root.add_child(rig)
	var player := rig.get_node("AnimationPlayer") as AnimationPlayer
	player.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL
	var records: Array = []
	for mode_index in range(4):
		var mode: String = MODES[mode_index]
		var strip := Image.create(256,48,false,Image.FORMAT_RGBA8)
		for variant in range(3): strip.blit_rect(atlases[variant],Rect2i(0,(4+mode_index)*16,256,16),Vector2i(0,variant*16))
		# Display over black with the renderer's linear-light convention, not the
		# image inspector's white transparency background. Alpha metrics use atlases.
		for y in range(48):
			for x in range(256):
				var p := strip.get_pixel(x,y)
				var c := p.srgb_to_linear()
				strip.set_pixel(x,y,Color(c.r*p.a,c.g*p.a,c.b*p.a,1).linear_to_srgb())
		assert(strip.save_png(args[3].path_join(mode+"-native.png")) == OK)
		strip.resize(2048,384,Image.INTERPOLATE_NEAREST)
		assert(strip.save_png(args[3].path_join(mode+"-8x.png")) == OK)
		for frame in range(16):
			player.play("RESET"); player.advance(0); player.play(mode)
			player.seek(float(frame)/(15 if mode == "bud" else 16)*player.get_animation(mode).length,true)
			rig.get_node("LeftFin/Sprite").visible = false
			rig.get_node("RightFin/Sprite").visible = false
			var body := raster_point(rig)
			rig.get_node("LeftFin/Sprite").visible = true
			rig.get_node("RightFin/Sprite").visible = true
			var variants: Array = []
			for variant in range(3):
				var whole_solid := 0
				var fin_solid := 0
				var fin_half := 0
				var fin_area := 0.0
				for y in range(16):
					for x in range(16):
						var pixel := atlases[variant].get_pixel(frame*16+x,(4+mode_index)*16+y)
						if pixel.a >= .9: whole_solid += 1
						if body.get_pixel(x,y).a == 0.0:
							fin_area += pixel.a
							if pixel.a >= .9: fin_solid += 1
							if pixel.a >= .5: fin_half += 1
				# Original move uses a different body mask; don't mislabel that partition.
				variants.append({"whole_solid":whole_solid,"fin_solid":null if variant == 0 and mode == "move" else fin_solid,"fin_half":null if variant == 0 and mode == "move" else fin_half,"fin_area":null if variant == 0 and mode == "move" else fin_area})
			records.append({"state":mode,"frame":frame,"variants":variants})
	var file := FileAccess.open(args[3].path_join("source-solid.json"),FileAccess.WRITE)
	file.store_string(JSON.stringify({"order":["original","body-hold-only","stable-body-plus-fin4"],"mask":"Pixels outside the actual point-baked Body+Bud image at this pose. No RGB-color heuristic. Original move partition omitted because its body differs.","frames":records},"  "))
	quit()
