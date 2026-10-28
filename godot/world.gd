extends Node2D
## Rust owns simulation and collision. This node renders its world-space snapshot.

const SCALE := 24.0
const INK := Color("dbe8ee")
const MUTED := Color("8b9daa")
const WALL := Color("72545c")
const FLOW := Color(0.4, 0.8, 0.77, 0.5)

@onready var floor_mesh: MeshInstance2D = $Floor
@onready var landmark_backgrounds: Node2D = $LandmarkBackgrounds
@onready var camera: Camera2D = $Camera
var hud

var simulation := BloodLab.new()
var state: Dictionary = {}
var wall_segments := PackedVector2Array()
var landmarks: Array = []
var show_flow := false
var show_collision := false
var font := ThemeDB.fallback_font


func _ready() -> void:
	hud = $HUDLayer/HUD
	var args := OS.get_cmdline_user_args()
	var use_lbm := args.has("--local-lbm")
	var use_stokes := args.has("--local-stokes")
	var profile := "fine"
	var profile_given := false
	for arg in args:
		if arg.begins_with("--lbm-profile="):
			if profile_given:
				_startup_error("LBM profile specified more than once")
				return
			profile = arg.trim_prefix("--lbm-profile=")
			profile_given = true
	if use_lbm and use_stokes:
		_startup_error("Choose either --local-lbm or --local-stokes")
		return
	if profile_given and not use_lbm:
		_startup_error("--lbm-profile requires --local-lbm")
		return
	if use_lbm:
		if not simulation.use_lbm_world(profile):
			_startup_error(simulation.snapshot().get("error", "LBM startup failed"))
			return
	elif use_stokes:
		simulation.use_local_world()
	else:
		simulation.use_world()
	var geometry: Dictionary = simulation.world_geometry()
	landmarks = geometry.get("landmarks", [])
	_build_walls(geometry.get("walls", []))
	_build_landmark_backgrounds()
	_build_floor(geometry.get("floor", PackedVector2Array()))
	hud.pause_requested.connect(_toggle_pause)
	hud.step_requested.connect(_single_step)
	hud.reset_requested.connect(_reset)
	hud.parameters_changed.connect(_parameters_changed)
	hud.flow_changed.connect(_flow_changed)
	hud.collision_changed.connect(_collision_changed)
	_refresh()
	camera.position = state.robot * SCALE


func _startup_error(message: String) -> void:
	push_error("Startup failed: " + message)
	set_physics_process(false)
	get_tree().quit(1)


func _build_floor(floor: PackedVector2Array) -> void:
	var vertices := PackedVector3Array()
	vertices.resize(floor.size())
	for i in floor.size():
		vertices[i] = Vector3(floor[i].x * SCALE, floor[i].y * SCALE, 0.0)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	var mesh := ArrayMesh.new()
	if vertices.size() > 0:
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	floor_mesh.mesh = mesh
	floor_mesh.modulate = Color("34272b")


func _build_walls(walls: Array) -> void:
	for wall in walls:
		for i in range(wall.size() - 1):
			wall_segments.append(wall[i] * SCALE)
			wall_segments.append(wall[i + 1] * SCALE)


func _build_landmark_backgrounds() -> void:
	for landmark in landmarks:
		var position: Vector2 = landmark.position * SCALE
		var half_size: Vector2 = landmark.size * SCALE * 0.5
		var background := Polygon2D.new()
		background.polygon = PackedVector2Array([
			position + Vector2(-half_size.x, -half_size.y),
			position + Vector2(half_size.x, -half_size.y),
			position + Vector2(half_size.x, half_size.y),
			position + Vector2(-half_size.x, half_size.y),
		])
		background.color = Color("222b30")
		landmark_backgrounds.add_child(background)


func _physics_process(_delta: float) -> void:
	simulation.tick(_movement())
	_refresh()


func _movement() -> Vector2:
	return Vector2(
		float(Input.is_physical_key_pressed(KEY_D) or Input.is_physical_key_pressed(KEY_RIGHT)) - float(Input.is_physical_key_pressed(KEY_A) or Input.is_physical_key_pressed(KEY_LEFT)),
		float(Input.is_physical_key_pressed(KEY_S) or Input.is_physical_key_pressed(KEY_DOWN)) - float(Input.is_physical_key_pressed(KEY_W) or Input.is_physical_key_pressed(KEY_UP))
	).limit_length()


func _unhandled_key_input(event: InputEvent) -> void:
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
	for landmark in landmarks:
		var position: Vector2 = landmark.position * SCALE
		var size: Vector2 = landmark.size * SCALE
		draw_rect(Rect2(position - size * 0.5, size), Color("52616a"), false, 2.0)
		draw_string(font, position + Vector2(-size.x * 0.42, -size.y * 0.28), landmark.name, HORIZONTAL_ALIGNMENT_LEFT, -1, 20, INK)
	if not wall_segments.is_empty():
		draw_multiline(wall_segments, WALL, 2.0, true)
		if show_collision:
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
	for i in state.cells.size():
		var contour := _scaled(state.cells[i])
		if contour.size() < 3:
			continue
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
