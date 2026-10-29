extends SceneTree
func _initialize() -> void:
	_run.call_deferred()
func _run() -> void:
	var world = load("res://world.tscn").instantiate()
	root.add_child(world)
	var intervals: Array[float] = []
	var draw_calls: Array[float] = []
	var last := 0
	for i in 780:
		await physics_frame
		var now := Time.get_ticks_usec()
		if int(world.state.steps) >= 180 and int(world.state.steps) < 780 and last > 0:
			intervals.append((now-last)/1000.0)
			draw_calls.append(float(RenderingServer.get_rendering_info(RenderingServer.RENDERING_INFO_TOTAL_DRAW_CALLS_IN_FRAME)))
		last = now
		if i == 119:
			await RenderingServer.frame_post_draw
			var error := root.get_texture().get_image().save_png(OS.get_environment("CELL_CAPTURE"))
			print("CAPTURE ",error," steps=",world.state.steps," robot=",world.state.robot)
	await RenderingServer.frame_post_draw
	var result := {"draw_ms": _stats(world.draw_samples), "frame_ms":_stats(intervals), "draw_calls":_stats(draw_calls), "steps":world.state.steps, "robot":str(world.state.robot)}
	print("CELL_RENDER_RESULT ",JSON.stringify(result))
	quit()
func _stats(values: Array[float]) -> Dictionary:
	values.sort()
	var total := 0.0
	for value in values:
		total += value
	return {"n":values.size(), "mean":total/values.size(), "p95":values[int(ceil(values.size()*0.95))-1], "max":values[-1]}
