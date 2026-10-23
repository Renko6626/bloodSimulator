extends Control
## Screen-space controls and status for the world scene.

signal pause_requested
signal step_requested
signal reset_requested
signal parameters_changed(pressure: float, speed: float, drag: float)
signal flow_changed(value: bool)
signal collision_changed(value: bool)

const INK := Color("dbe8ee")
const MUTED := Color("8b9daa")

var stats: Label
var flows: Label
var pause_button: Button
var step_button: Button
var pressure: HSlider
var speed: HSlider
var drag: HSlider
var flow_toggle: CheckButton
var collision_toggle: CheckButton


func _ready() -> void:
	_build_panel()
	var title := _add_label(self, "BLOOD SIMULATOR", 15, MUTED)
	title.position = Vector2(24, 20)
	var subtitle := _add_label(self, "World circulation", 27, INK)
	subtitle.position = Vector2(24, 46)
	var help := _add_label(self, "WASD / arrows: move    Space: pause    N: step    R: reset    F: flow    C: collision", 14, INK)
	help.anchor_top = 1.0
	help.anchor_bottom = 1.0
	help.offset_left = 24.0
	help.offset_top = -36.0
	help.offset_right = 940.0
	help.offset_bottom = -12.0


func _build_panel() -> void:
	var panel := PanelContainer.new()
	panel.anchor_left = 1.0
	panel.anchor_right = 1.0
	panel.anchor_bottom = 1.0
	panel.offset_left = -278.0
	panel.offset_top = 20.0
	panel.offset_right = -20.0
	panel.offset_bottom = -20.0
	var style := StyleBoxFlat.new()
	style.bg_color = Color("1b1b1d")
	style.border_color = Color("373337")
	style.set_border_width_all(1)
	style.content_margin_left = 16
	style.content_margin_right = 16
	style.content_margin_top = 16
	style.content_margin_bottom = 16
	panel.add_theme_stylebox_override("panel", style)
	add_child(panel)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 7)
	panel.add_child(box)
	_add_label(box, "WORLD MAP", 13, MUTED)
	_add_label(box, "Heart → two organs → heart", 17, INK)
	stats = _add_label(box, "", 13, INK)
	stats.add_theme_constant_override("line_spacing", 2)
	_add_label(box, "Branch flow", 13, MUTED)
	var flow_scroll := ScrollContainer.new()
	flow_scroll.custom_minimum_size.y = 130.0
	flow_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	flow_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	box.add_child(flow_scroll)
	flows = _add_label(flow_scroll, "", 12, MUTED)
	flows.add_theme_constant_override("line_spacing", 2)
	pause_button = _button(box, "Pause [Space]", func(): pause_requested.emit())
	step_button = _button(box, "Single step [N]", func(): step_requested.emit())
	_button(box, "Reset [R]", func(): reset_requested.emit())
	pressure = _slider(box, "Pump pressure", 0.0, 30.0, 12.0)
	speed = _slider(box, "Robot speed", 1.0, 8.0, 4.0)
	drag = _slider(box, "Cell flow drag", 0.0, 40.0, 20.0)
	for slider in [pressure, speed, drag]:
		slider.value_changed.connect(_parameters_changed)
	flow_toggle = CheckButton.new()
	flow_toggle.text = "Flow direction [F]"
	flow_toggle.focus_mode = Control.FOCUS_NONE
	flow_toggle.toggled.connect(func(value: bool): flow_changed.emit(value))
	box.add_child(flow_toggle)
	collision_toggle = CheckButton.new()
	collision_toggle.text = "Collision surface [C]"
	collision_toggle.focus_mode = Control.FOCUS_NONE
	collision_toggle.toggled.connect(func(value: bool): collision_changed.emit(value))
	box.add_child(collision_toggle)


func update_state(state: Dictionary) -> void:
	pause_button.text = "Resume [Space]" if state.paused else "Pause [Space]"
	step_button.disabled = not state.paused
	stats.text = "Cells                  %d\nTime                   %.1f s\nSolve / step           %.2f ms\nSoft contacts          %d\n\nPressure drop          %.2f\nPump flow              %.2f\nMean speed             %.2f" % [state.cells.size(), state.time, state.solve_ms, state.soft_contacts, state.pressure_drop, state.flow_rate, state.mean_speed]
	var lines := PackedStringArray()
	for i in state.branch_flows.size():
		lines.append("Vessel %02d             %7.2f" % [i + 1, state.branch_flows[i]])
	flows.text = "\n".join(lines)


func toggle_flow() -> void:
	flow_toggle.button_pressed = not flow_toggle.button_pressed


func toggle_collision() -> void:
	collision_toggle.button_pressed = not collision_toggle.button_pressed


func _parameters_changed(_value: float) -> void:
	parameters_changed.emit(pressure.value, speed.value, drag.value)


func _add_label(parent: Node, value: String, size: int, color: Color) -> Label:
	var label := Label.new()
	label.text = value
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	parent.add_child(label)
	return label


func _button(parent: Node, value: String, action: Callable) -> Button:
	var button := Button.new()
	button.text = value
	button.custom_minimum_size.y = 32
	button.focus_mode = Control.FOCUS_NONE
	button.pressed.connect(action)
	parent.add_child(button)
	return button


func _slider(parent: Node, name: String, low: float, high: float, initial: float) -> HSlider:
	var label := _add_label(parent, "%s  %.1f" % [name, initial], 13, MUTED)
	var slider := HSlider.new()
	slider.min_value = low
	slider.max_value = high
	slider.step = 0.1
	slider.value = initial
	slider.focus_mode = Control.FOCUS_NONE
	slider.value_changed.connect(func(value: float): label.text = "%s  %.1f" % [name, value])
	parent.add_child(slider)
	return slider
