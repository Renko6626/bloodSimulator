extends Node2D
## Rust owns simulation and collision. This node renders its world-space snapshot.

const SCALE := 24.0
const Background = preload("res://world_background.gd")
const FLOW := Color(0.4, 0.8, 0.77, 0.5)

@onready var background: Background = $StaticBackground
@onready var camera: Camera2D = $Camera
var hud

var simulation := BloodLab.new()
var state: Dictionary = {}
var wall_segments := PackedVector2Array()
var show_flow := false
var show_collision := false
var font := ThemeDB.fallback_font
var benchmark := ""
var benchmark_frames := 3600
var benchmark_warmup := 120
var benchmark_frame := 0
var benchmark_output := ""
var benchmark_file: FileAccess
var render_file: FileAccess
var previous_render_usec := 0
var screenshot_path := ""
var screenshot_started := false
var previous_centers: Array[Vector2] = []



func _ready() -> void:
	hud = $HUDLayer/HUD
	var args := OS.get_cmdline_user_args()
	var use_lbm := args.has("--local-lbm")
	var use_stokes := args.has("--local-stokes")
	var local_cells := args.has("--local-cells")
	var validation := args.has("--lbm-validation")
	var profile := "fine"
	var profile_given := false
	for arg in args:
		if arg.begins_with("--lbm-profile="):
			if profile_given:
				_startup_error("LBM profile specified more than once")
				return
			profile = arg.trim_prefix("--lbm-profile=")
			profile_given = true
	for arg in args:
		if arg.begins_with("--lbm-benchmark="):
			benchmark = arg.trim_prefix("--lbm-benchmark=")
		elif arg.begins_with("--lbm-benchmark-frames="):
			benchmark_frames = arg.trim_prefix("--lbm-benchmark-frames=").to_int()
		elif arg.begins_with("--lbm-benchmark-warmup="):
			benchmark_warmup = arg.trim_prefix("--lbm-benchmark-warmup=").to_int()
		elif arg.begins_with("--lbm-benchmark-output="):
			benchmark_output = arg.trim_prefix("--lbm-benchmark-output=")
		elif arg.begins_with("--lbm-screenshot="):
			screenshot_path = arg.trim_prefix("--lbm-screenshot=")
	if validation and not use_lbm:
		_startup_error("--lbm-validation requires --local-lbm")
		return
	if not benchmark.is_empty() and (not validation or not benchmark in ["hold", "move"] or benchmark_frames < 1 or benchmark_warmup < 0 or benchmark_output.is_empty()):
		_startup_error("benchmark needs --local-lbm --lbm-validation, mode hold/move and an output CSV path")
		return
	if not screenshot_path.is_empty() and (benchmark.is_empty() or benchmark_warmup < 2):
		_startup_error("--lbm-screenshot requires benchmark warmup >=2")
		return
	if use_lbm and use_stokes:
		_startup_error("Choose either --local-lbm or --local-stokes")
		return
	if profile_given and not use_lbm:
		_startup_error("--lbm-profile requires --local-lbm")
		return
	if local_cells and use_stokes:
		_startup_error("--local-cells supports world and LBM; local Stokes is unavailable")
		return
	simulation.configure_local_cells(local_cells)
	if use_lbm:
		var created: bool = simulation.use_lbm_validation(profile) if validation else simulation.use_lbm_world(profile)
		if not created:
			_startup_error(simulation.snapshot().get("error", "LBM startup failed"))
			return
	elif use_stokes:
		simulation.use_local_world()
	else:
		simulation.use_world()
	var geometry: Dictionary = simulation.world_geometry()
	_build_walls(geometry.get("walls", []))
	var background_error := background.configure(geometry, SCALE, font)
	if not background_error.is_empty():
		_startup_error(background_error)
		return
	hud.pause_requested.connect(_toggle_pause)
	hud.step_requested.connect(_single_step)
	hud.reset_requested.connect(_reset)
	hud.parameters_changed.connect(_parameters_changed)
	hud.flow_changed.connect(_flow_changed)
	hud.collision_changed.connect(_collision_changed)
	_refresh()
	camera.position = state.robot * SCALE
	if validation:
		hud.set_pressure_display(state.pressure_drop)
	if not benchmark.is_empty():
		_begin_benchmark()


func _startup_error(message: String) -> void:
	push_error("Startup failed: " + message)
	set_physics_process(false)
	get_tree().quit(1)


func _build_walls(walls: Array) -> void:
	for wall in walls:
		for i in range(wall.size() - 1):
			wall_segments.append(wall[i] * SCALE)
			wall_segments.append(wall[i + 1] * SCALE)


func _physics_process(_delta: float) -> void:
	var started := Time.get_ticks_usec()
	var movement := _movement()
	var tick_start := Time.get_ticks_usec()
	simulation.tick(movement)
	var tick_ms := (Time.get_ticks_usec() - tick_start) / 1000.0
	var snapshot_start := Time.get_ticks_usec()
	_refresh()
	var snapshot_ms := (Time.get_ticks_usec() - snapshot_start) / 1000.0
	if not benchmark.is_empty():
		_benchmark_row(movement, tick_ms, snapshot_ms, (Time.get_ticks_usec() - started) / 1000.0)
		if not screenshot_path.is_empty() and not screenshot_started and benchmark_frame == mini(60, int(benchmark_warmup / 2)):
			screenshot_started = true
			_capture_benchmark_screenshot()


func _process(_delta: float) -> void:
	if render_file == null:
		return
	var started := Time.get_ticks_usec()
	var interval_ms := 0.0 if previous_render_usec == 0 else (started - previous_render_usec) / 1000.0
	previous_render_usec = started
	render_file.store_csv_line(PackedStringArray([
		str(Engine.get_process_frames()), str(benchmark_frame), str(interval_ms),
		str(RenderingServer.get_rendering_info(RenderingServer.RENDERING_INFO_TOTAL_DRAW_CALLS_IN_FRAME)),
		str(DisplayServer.get_name()), str(RenderingServer.get_current_rendering_method()),
		str((Time.get_ticks_usec() - started) / 1000.0)
	]))


func _movement() -> Vector2:
	if not benchmark.is_empty():
		var target := Vector2(24, 24)
		if benchmark == "move" and benchmark_frame >= benchmark_warmup:
			target.x = 10.0 if (int((benchmark_frame - benchmark_warmup) / 720) % 2 == 0) else 28.0
		return ((target - Vector2(state.robot)) * 2.0).limit_length()
	return Vector2(
		float(Input.is_physical_key_pressed(KEY_D) or Input.is_physical_key_pressed(KEY_RIGHT)) - float(Input.is_physical_key_pressed(KEY_A) or Input.is_physical_key_pressed(KEY_LEFT)),
		float(Input.is_physical_key_pressed(KEY_S) or Input.is_physical_key_pressed(KEY_DOWN)) - float(Input.is_physical_key_pressed(KEY_W) or Input.is_physical_key_pressed(KEY_UP))
	).limit_length()


func _unhandled_key_input(event: InputEvent) -> void:
	if not benchmark.is_empty():
		return
	if event is InputEventKey and event.pressed and not event.echo:
		match event.physical_keycode:
			KEY_SPACE: _toggle_pause()
			KEY_N: _single_step()
			KEY_R: _reset()
			KEY_F: hud.toggle_flow()
			KEY_C: hud.toggle_collision()


func _toggle_pause() -> void:
	simulation.set_paused(not state.get("paused", false))
	_refresh()


func _single_step() -> void:
	simulation.single_step(_movement())
	_refresh()


func _reset() -> void:
	simulation.reset()
	_refresh()


func _parameters_changed(pressure: float, speed: float, drag: float) -> void:
	simulation.set_parameters(pressure, speed, drag)


func _flow_changed(value: bool) -> void:
	show_flow = value
	queue_redraw()


func _collision_changed(value: bool) -> void:
	show_collision = value
	queue_redraw()


func _refresh() -> void:
	state = simulation.snapshot()
	camera.position = state.robot * SCALE
	hud.update_state(state)
	queue_redraw()


func _scaled(source: PackedVector2Array) -> PackedVector2Array:
	var result := PackedVector2Array()
	result.resize(source.size())
	for i in source.size():
		result[i] = source[i] * SCALE
	return result


func _draw() -> void:
	if state.is_empty():
		return
	if show_collision and not wall_segments.is_empty():
		draw_multiline(wall_segments, Color("f3d59b"), 0.16 * SCALE, true)
	if show_flow:
		for arrow in state.arrows:
			var start: Vector2 = arrow[0] * SCALE
			var velocity: Vector2 = arrow[1]
			if velocity.length_squared() < 0.0001:
				continue
			var end := start + velocity * 23.0
			draw_line(start, end, FLOW, 1.5, true)
			var direction := velocity.normalized()
			draw_line(end, end - direction.rotated(0.5) * 5.0, FLOW, 1.5, true)
			draw_line(end, end - direction.rotated(-0.5) * 5.0, FLOW, 1.5, true)
	var local_view: Rect2 = get_global_transform_with_canvas().affine_inverse() * get_viewport_rect()
	var visible_cells := Rect2(local_view.position / SCALE, local_view.size / SCALE).grow(8.0 / SCALE)
	for i in state.cells.size():
		var cell: PackedVector2Array = state.cells[i]
		if cell.size() < 3:
			continue
		var cell_bounds := Rect2(cell[0], Vector2.ZERO)
		for point in cell:
			cell_bounds = cell_bounds.expand(point)
		if not visible_cells.intersects(cell_bounds, true):
			continue
		var contour := _scaled(cell)
		var center := Vector2.ZERO
		for point in contour:
			center += point
		center /= contour.size()
		draw_colored_polygon(contour, Color("bc754e") if i == 0 else Color("ae4c5b"))
		var closed := contour.duplicate()
		closed.append(contour[0])
		draw_polyline(closed, Color("d28189"), 1.0, true)
		var inner := PackedVector2Array()
		for point in contour:
			inner.append(center + (point - center) * 0.48)
		draw_colored_polygon(inner, Color("6f293e"))
		if show_collision:
			draw_polyline(closed, Color("f3d59b"), 0.14 * SCALE, true)
	var robot: Vector2 = state.robot * SCALE
	draw_circle(robot, 0.85 * SCALE, Color("aaaaae"))
	draw_arc(robot, 0.85 * SCALE, 0.0, TAU, 32, Color("e0e0e2"), 1.5, true)


func _cell_centers() -> Array[Vector2]:
	var centers: Array[Vector2] = []
	for cell in state.cells:
		var center := Vector2.ZERO
		for p in cell:
			center += p
		centers.append(center / cell.size())
	return centers


func _begin_benchmark() -> void:
	benchmark_file = FileAccess.open(benchmark_output, FileAccess.WRITE)
	render_file = FileAccess.open(benchmark_output + ".render.csv", FileAccess.WRITE)
	if benchmark_file == null or render_file == null:
		_startup_error("Cannot create benchmark output files")
		return
	previous_centers = _cell_centers()
	benchmark_file.store_csv_line(PackedStringArray([
		"phase", "frame", "time", "input_x", "input_y", "robot_x", "robot_y", "active_cells", "nearby_cells8", "soft_contacts",
		"cell_center_speed_mean", "cell_center_speed_max", "cell_displacement_mean", "cell_displacement_max", "area_ratio_min", "area_ratio_max", "node_wall_max", "robot_wall", "finite",
		"density_min", "density_max", "density_sum", "targets", "fluxes", "region_switches", "origin_x", "origin_y", "overlap_error", "branch_flows",
		"prepare_ms", "coupling_ms", "lbm_ms", "rapier_ms", "step_wall_ms", "tick_wall_ms", "snapshot_refresh_ms", "physics_callback_before_csv_ms",
		"lattice_steps", "dispatches", "submissions", "readback_calls", "readback_bytes", "readback_wait_ms", "total_cells", "detailed_cells", "abstract_cells", "spawned", "retired", "cell_exchange_ms"
	]))
	render_file.store_csv_line(PackedStringArray(["process_frame", "physics_frame", "render_interval_ms", "draw_calls_previous_frame", "display_server", "rendering_method", "process_callback_before_csv_ms"]))
	print("LBM_GAMEPLAY_BENCH mode=", benchmark, " warmup=", benchmark_warmup, " frames=", benchmark_frames, " renderer=", RenderingServer.get_current_rendering_method(), " display=", DisplayServer.get_name(), " adapter=", state.lbm.get("adapter_name", ""), " software=", state.lbm.get("software_adapter", true))


func _joined(values: Array) -> String:
	var fields := PackedStringArray()
	for value in values:
		fields.append(str(value))
	return "|".join(fields)


func _benchmark_row(movement: Vector2, tick_ms: float, snapshot_ms: float, callback_ms: float) -> void:
	var l: Dictionary = state.get("lbm", {})
	var q: Dictionary = state.get("validation", {})
	var population: Dictionary = state.get("population", {})
	var centers := _cell_centers()
	var speed_sum := 0.0
	var speed_max := 0.0
	if population.is_empty():
		for i in centers.size():
			var speed := centers[i].distance_to(previous_centers[i]) * 60.0
			speed_sum += speed
			speed_max = maxf(speed_max, speed)
	previous_centers = centers
	var values: Array = [
		"warmup" if benchmark_frame < benchmark_warmup else "measure", benchmark_frame, state.time, movement.x, movement.y, state.robot.x, state.robot.y,
		q.get("active_cells", 0), q.get("nearby_cells8", 0), state.soft_contacts, speed_sum / max(1, centers.size()) if population.is_empty() else "", speed_max if population.is_empty() else "",
		q.get("cell_displacement_mean", ""), q.get("cell_displacement_max", ""), q.get("area_ratio_min", 0) if not centers.is_empty() else "", q.get("area_ratio_max", 0) if not centers.is_empty() else "", q.get("node_wall_max", 0) if not centers.is_empty() else "", q.get("robot_wall", 0), q.get("finite", false),
		l.get("density_min", 0), l.get("density_max", 0), l.get("mass", 0), _joined(l.get("opening_targets", [])), _joined(l.get("opening_fluxes", [])),
		l.get("region_switches", 0), l.get("origin_blocks", Vector2i.ZERO).x, l.get("origin_blocks", Vector2i.ZERO).y, l.get("overlap_error", 0), _joined(state.branch_flows),
		q.get("prepare_ms", 0), l.get("coupling_ms", 0), l.get("lbm_ms", 0), l.get("rapier_ms", 0), state.solve_ms, tick_ms, snapshot_ms, callback_ms,
		q.get("lattice_steps", 0), q.get("dispatches", 0), q.get("submissions", 0), q.get("readback_calls", 0), q.get("readback_bytes", 0), q.get("readback_wait_ms", 0),
		population.get("total_count", ""), population.get("detailed_count", ""), population.get("abstract_count", ""),
		population.get("spawned", ""), population.get("retired", ""), population.get("exchange_ms", "")
	]
	var fields := PackedStringArray()
	for value in values:
		fields.append(str(value))
	benchmark_file.store_csv_line(fields)
	benchmark_frame += 1
	var failure: String = state.get("error", "")
	if failure.is_empty():
		failure = state.get("lbm_status", "")
	if failure.is_empty():
		failure = q.get("guard_error", "")
	if not population.is_empty() and absf(float(population.detailed_count) + float(population.abstract_count) - float(population.total_count)) > 0.000001:
		failure = "cell population quantity mismatch"
	for residual in l.get("opening_residuals", []):
		if absf(residual) > 0.001:
			failure = "opening flux residual exceeds 0.001"
	if not failure.is_empty() or benchmark_frame >= benchmark_warmup + benchmark_frames:
		if failure.is_empty() and benchmark == "move" and benchmark_frames >= 1440 and l.get("region_switches", 0) == 0:
			failure = "route did not produce an actual migration"
		benchmark_file.flush()
		render_file.flush()
		var status := FileAccess.open(benchmark_output + ".status.txt", FileAccess.WRITE)
		if status != null:
			status.store_string("COMPLETE" if failure.is_empty() else "FAILED " + failure)
		print("LBM_GAMEPLAY_BENCH_END frames=", benchmark_frame, " robot=", state.robot, " shifts=", l.get("region_switches", 0), " error=", failure)
		set_physics_process(false)
		get_tree().quit(0 if failure.is_empty() else 1)
	elif benchmark_frame % 120 == 0:
		benchmark_file.flush()
		render_file.flush()


func _capture_benchmark_screenshot() -> void:
	await RenderingServer.frame_post_draw
	var error := get_viewport().get_texture().get_image().save_png(screenshot_path)
	print("LBM_GAMEPLAY_SCREENSHOT path=", screenshot_path, " error=", error, " phase=warmup")
