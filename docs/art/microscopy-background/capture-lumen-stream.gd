extends SceneTree
func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	var scene = load("res://world.tscn").instantiate()
	root.add_child(scene)
	await process_frame
	scene.set_physics_process(false)
	scene.camera.zoom = Vector2(0.75, 0.75)
	scene.camera.position_smoothing_enabled = false
	scene.queue_redraw()
	for i in 12: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/lumen-stream.png")
	quit()
