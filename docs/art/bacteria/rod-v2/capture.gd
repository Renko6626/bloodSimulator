extends SceneTree
## Static art capture. No input events or UI interaction are synthesized.

var scene
const OUT := "/tmp/sunyunbo/bacteria-rod-v2-20261105/"

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	scene = load("res://world.tscn").instantiate()
	root.add_child(scene)
	scene.set_physics_process(false)
	await process_frame
	var motile := -1
	var encapsulated := -1
	for i in scene.state.cells.size():
		if scene.state.cell_status[i].species == "motile_bacterium" and motile < 0:
			motile = i
		if scene.state.cell_status[i].species == "encapsulated_bacterium" and encapsulated < 0:
			encapsulated = i
	assert(motile >= 0 and encapsulated >= 0)
	var mid := (_center(scene.state.cells[motile]) + _center(scene.state.cells[encapsulated])) * 12.0
	scene.camera.position = mid + Vector2(70, 0)
	scene.camera.zoom = Vector2.ONE
	await _save("game-default.png")
	scene.camera.zoom = Vector2(2, 2)
	await _save("game-close.png")
	scene.get_node("HUDLayer").hide()
	scene.camera.position = _center(scene.state.cells[motile]) * 24.0 + Vector2(9, 0)
	scene.camera.zoom = Vector2(12, 12)
	await _save("motile-detail.png")
	scene.state.time = 0.125
	# Repeated rendering at an unchanged snapshot time must keep the same tail pose.
	scene.camera.zoom = Vector2(6, 6)
	scene.camera.position = _center(scene.state.cells[motile]) * 24.0 + Vector2(640.0 / 6.0 + 0.55 * 24.0, 0)
	scene.camera.position = _center(scene.state.cells[encapsulated]) * 24.0
	scene.camera.zoom = Vector2(10, 10)
	# Display-only diagnostic snapshots: exercise the production drawing path.
	# These images are explicitly not claims about simulated corpse/deformation states.
	scene.state.cell_status[encapsulated].dead = true
	scene.state.cell_status[encapsulated].opacity = 0.5
	scene.camera.position = _center(scene.state.cells[motile]) * 24.0 + Vector2(9, 0)
	scene.camera.zoom = Vector2(12, 12)
	var points: PackedVector2Array = scene.state.cells[motile].duplicate()
	var center := _center(points)
	for i in points.size():
		var p: Vector2 = points[i] - center
		points[i] = center + Vector2(p.x * 0.8, p.y * 1.25).rotated(0.5)
	scene.state.cells[motile] = points
	await _save("motile-deformed-swatch.png")
	scene.state.cell_status[motile].dead = true
	scene.state.cell_status[motile].opacity = 0.5
	print("BACTERIA_ART_CAPTURE PASS")
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
	var error := root.get_texture().get_image().save_png(OUT + name)
	assert(error == OK)
