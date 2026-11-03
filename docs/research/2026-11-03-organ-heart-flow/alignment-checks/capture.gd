extends SceneTree

func _initialize() -> void:
	capture.call_deferred()

func capture() -> void:
	root.size = Vector2i(1280, 800)
	var world = load("res://world.tscn").instantiate()
	root.add_child(world)
	for frame in range(30):
		await process_frame
	await RenderingServer.frame_post_draw
	var result := root.get_texture().get_image().save_png(OS.get_environment("HEART_CAPTURE_PATH"))
	print("Static organ background capture exit: ", result)
	quit(0 if result == OK else 1)
