extends Node2D
## Input and drawing only. Rust owns every moving object's physics state.

const SCALE := 24.0 # pixels per local simulation unit
const ORIGIN := Vector2(235.0, 235.0)
const INK := Color("dbe8ee")
const MUTED := Color("8b9daa")
const TEAL := Color("77d8cd")

var simulation := BloodLab.new()
var state: Dictionary = {}
var walls: Array = []
var stats: Label
var pause_button: Button
var step_button: Button
var pressure: HSlider
var speed: HSlider
var drag: HSlider
var flow_toggle: CheckButton
var collision_toggle: CheckButton
var show_flow := false
var show_collision := false
var font := ThemeDB.fallback_font


func _ready() -> void:
	walls = simulation.geometry()
	_build_panel()
	_refresh()


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
			KEY_F: flow_toggle.button_pressed = not flow_toggle.button_pressed
			KEY_C: collision_toggle.button_pressed = not collision_toggle.button_pressed


func _toggle_pause() -> void:
	simulation.set_paused(not state.get("paused", false))
	_refresh()


func _single_step() -> void:
	simulation.single_step(_movement())
	_refresh()


func _reset() -> void:
	simulation.reset()
	_refresh()


func _refresh() -> void:
	state = simulation.snapshot()
	pause_button.text = "Resume   [Space]" if state.paused else "Pause   [Space]"
	step_button.disabled = not state.paused
	stats.text = "CELLS                 %d\nSIM TIME              %.1f s\nSOLVE / STEP          %.2f ms\nSOFT CONTACTS         %d\n\nPRESSURE DROP         %.2f\nTOTAL FLOW            %.2f\nMEAN SPEED            %.2f" % [state.cells.size(), state.time, state.solve_ms, state.soft_contacts, state.pressure_drop, state.flow_rate, state.mean_speed]
	queue_redraw()


func _build_panel() -> void:
	var panel := PanelContainer.new()
	panel.position = Vector2(1000, 28)
	panel.size = Vector2(252, 744)
	var style := StyleBoxFlat.new()
	style.bg_color = Color("1b1b1d")
	style.border_color = Color("373337")
	style.set_border_width_all(1)
	style.content_margin_left = 18
	style.content_margin_right = 18
	style.content_margin_top = 18
	style.content_margin_bottom = 18
	panel.add_theme_stylebox_override("panel", style)
	add_child(panel)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 8)
	panel.add_child(box)
	_add_label(box, "CIRCULATION PROTOTYPE", 13, MUTED)
	_add_label(box, "One closed loop", 23, INK)
	stats = _add_label(box, "", 14, INK)
	stats.add_theme_constant_override("line_spacing", 3)
	pause_button = _button(box, "Pause", _toggle_pause)
	step_button = _button(box, "Single step   [N]", _single_step)
	_button(box, "Reset scene   [R]", _reset)
	pressure = _slider(box, "Pump pressure", 0.0, 30.0, 12.0)
	speed = _slider(box, "Robot speed", 1.0, 8.0, 4.0)
	drag = _slider(box, "Cell flow drag", 0.0, 40.0, 20.0)
	for slider in [pressure, speed, drag]:
		slider.value_changed.connect(_parameters_changed)
	flow_toggle = CheckButton.new()
	flow_toggle.text = "Flow directions   [F]"
	flow_toggle.button_pressed = show_flow
	flow_toggle.toggled.connect(func(value: bool): show_flow = value)
	box.add_child(flow_toggle)
	collision_toggle = CheckButton.new()
	collision_toggle.text = "Collision surface   [C]"
	collision_toggle.toggled.connect(func(value: bool): show_collision = value)
	box.add_child(collision_toggle)
	_add_label(box, "WASD / Arrows to move\nGray circle is the robot.\nF shows the flow direction.", 13, MUTED)


func _add_label(parent: Node, text: String, size: int, color: Color) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	parent.add_child(label)
	return label


func _button(parent: Node, text: String, action: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.custom_minimum_size.y = 34
	button.focus_mode = Control.FOCUS_NONE
	button.pressed.connect(action)
	parent.add_child(button)
	return button


func _slider(parent: Node, text: String, low: float, high: float, initial: float) -> HSlider:
	var label := _add_label(parent, "%s  %.1f" % [text, initial], 14, MUTED)
	var slider := HSlider.new()
	slider.min_value = low
	slider.max_value = high
	slider.step = 0.1
	slider.value = initial
	slider.focus_mode = Control.FOCUS_NONE
	slider.value_changed.connect(func(value: float): label.text = "%s  %.1f" % [text, value])
	parent.add_child(slider)
	return slider


func _parameters_changed(_value: float) -> void:
	simulation.set_parameters(pressure.value, speed.value, drag.value)


func _screen(point: Vector2) -> Vector2:
	return ORIGIN + point * SCALE


func _screen_points(source: PackedVector2Array) -> PackedVector2Array:
	var result := PackedVector2Array()
	for point in source:
		result.append(_screen(point))
	return result


func _draw() -> void:
	if state.is_empty():
		return
	draw_string(font, Vector2(42, 48), "BLOOD SIMULATOR", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, MUTED)
	draw_string(font, Vector2(42, 87), "A single systemic circulation", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, INK)
	for i in range(walls.size()):
		var wall: PackedVector2Array = walls[i]
		var contour := _screen_points(wall)
		draw_colored_polygon(contour, Color("2c2024") if i == 0 else Color("151517"))
		draw_polyline(contour, Color("72545c"), 2.0, true)
		if show_collision:
			draw_polyline(contour, Color("f3d59b"), 0.16 * SCALE, true)
	draw_string(font, _screen(Vector2(6.0, 5.5)), "SYSTEMIC LOOP", HORIZONTAL_ALIGNMENT_LEFT, -1, 19, MUTED)
	draw_string(font, _screen(Vector2(5.0, 7.0)), "Outflow above. Return below.", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, MUTED)
	if show_flow:
		for arrow in state.arrows:
			var start := _screen(arrow[0])
			var velocity: Vector2 = arrow[1]
			var end := start + velocity * 23.0
			draw_line(start, end, Color(0.4, 0.8, 0.77, 0.45), 1.5, true)
			var direction := velocity.normalized()
			draw_line(end, end - direction.rotated(0.5) * 5.0, Color(0.4, 0.8, 0.77, 0.5), 1.5, true)
			draw_line(end, end - direction.rotated(-0.5) * 5.0, Color(0.4, 0.8, 0.77, 0.5), 1.5, true)
	for i in range(state.cells.size()):
		var cell: PackedVector2Array = state.cells[i]
		var contour := _screen_points(cell)
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
			for point in contour:
				draw_circle(point, 2.0, Color.WHITE)
	var robot: Vector2 = _screen(state.robot)
	draw_circle(robot, 0.85 * SCALE, Color("aaaaae"))
	draw_arc(robot, 0.85 * SCALE, 0, TAU, 32, Color("e0e0e2"), 1.5, true)
	draw_string(font, Vector2(43, 713), "WASD to move. Release to stop steering. A slight downstream drift remains.", HORIZONTAL_ALIGNMENT_LEFT, -1, 15, INK)
	draw_string(font, Vector2(43, 742), "The amber cell is a reference for watching a full lap. Space pauses; R resets.", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, MUTED)
