extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	var scene = load("res://world.tscn").instantiate()
	root.add_child(scene)
	await process_frame
	scene.set_physics_process(false)
	var center := Vector2.ZERO
	for i in scene.state.cells.size():
		if scene.state.cell_status[i].species == "neutrophil":
			for p in scene.state.cells[i]: center += p
			center /= scene.state.cells[i].size()
			break
	scene.camera.position = center * 24.0 + Vector2(100, 0)
	scene.camera.zoom = Vector2(2, 2)
	scene.queue_redraw()
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/lumen-game.png")
	quit()
