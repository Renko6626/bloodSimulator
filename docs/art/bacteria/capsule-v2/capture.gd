extends SceneTree
## Frozen scene screenshots; deformed/dead samples modify display snapshots only.

var scene
const OUT := "/tmp/sunyunbo/bacteria-capsule-v2-20261105/"

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	scene = load("res://world.tscn").instantiate()
	root.add_child(scene)
	scene.set_physics_process(false)
	await process_frame
	var encapsulated := -1
	var motile := -1
	for i in scene.state.cells.size():
		if scene.state.cell_status[i].species == "encapsulated_bacterium" and encapsulated < 0:
			encapsulated = i
		if scene.state.cell_status[i].species == "motile_bacterium" and motile < 0:
			motile = i
	assert(encapsulated >= 0 and motile >= 0)
	scene.camera.position = (_center(scene.state.cells[encapsulated]) + _center(scene.state.cells[motile])) * 12.0 + Vector2(70, 0)
	scene.camera.zoom = Vector2.ONE
	await _save("game-default.png")
	scene.camera.zoom = Vector2(2, 2)
	await _save("game-close.png")
	scene.get_node("HUDLayer").hide()
	scene.camera.position = _center(scene.state.cells[encapsulated]) * 24.0
	scene.camera.zoom = Vector2(10, 10)
	await _save("encapsulated-detail.png")
	var points: PackedVector2Array = scene.state.cells[encapsulated].duplicate()
	var center := _center(points)
	for i in points.size():
		var p: Vector2 = points[i] - center
		points[i] = center + Vector2(p.x * 1.25, p.y * 0.8).rotated(0.6)
	scene.state.cells[encapsulated] = points
	await _save("deformed-swatch.png")
	scene.state.cell_status[encapsulated].dead = true
	scene.state.cell_status[encapsulated].opacity = 0.5
	await _save("dead-swatch.png")
	print("CAPSULE_ART_CAPTURE PASS")
	quit()

func _center(points: PackedVector2Array) -> Vector2:
	var result := Vector2.ZERO
	for p in points:
		result += p
	return result / points.size()

func _save(name: String) -> void:
	scene.camera.reset_smoothing()
	scene.camera.force_update_scroll()
	scene.queue_redraw()
	for i in 6:
		await process_frame
	await RenderingServer.frame_post_draw
	assert(root.get_texture().get_image().save_png(OUT + name) == OK)
