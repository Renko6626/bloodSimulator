extends Node2D
## Static drawing commands used once by the background viewport.

const FLOOR := Color("34272b")
const LANDMARK_BACKGROUND := Color("222b30")
const LANDMARK_BORDER := Color("52616a")
const INK := Color("dbe8ee")
const WALL := Color("72545c")
const FONT_SIZE := 20

var floor_mesh: ArrayMesh
var wall_segments := PackedVector2Array()
var landmarks: Array = []
var pixels_per_unit := 24.0
var font: Font
var content_rect := Rect2()


func configure_chunk(chunk: Dictionary, source_landmarks: Array, scale_value: float, label_font: Font) -> void:
	var bounds: PackedVector2Array = chunk.bounds
	var visible := Rect2(bounds[0] * scale_value, (bounds[1] - bounds[0]) * scale_value).grow(8.0)
	var selected: Array = []
	for landmark in source_landmarks:
		var position: Vector2 = landmark.position * scale_value
		var size: Vector2 = landmark.size * scale_value
		var rect := Rect2(position - size * 0.5, size)
		var baseline := position + Vector2(-size.x * 0.42, -size.y * 0.28)
		var text_size := label_font.get_string_size(landmark.name, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE)
		rect = rect.merge(Rect2(baseline - Vector2(0, label_font.get_ascent(FONT_SIZE)), Vector2(text_size.x, label_font.get_height(FONT_SIZE))))
		if visible.intersects(rect, true):
			selected.append(landmark)
	configure({"bounds": chunk.paint_bounds, "floor": chunk.floor, "walls": chunk.walls, "landmarks": selected}, scale_value, label_font)


func has_content() -> bool:
	return floor_mesh != null or not wall_segments.is_empty() or not landmarks.is_empty()


func configure(geometry: Dictionary, scale_value: float, label_font: Font) -> void:
	pixels_per_unit = scale_value
	font = label_font
	landmarks = geometry.get("landmarks", [])
	var bounds: PackedVector2Array = geometry.bounds
	content_rect = Rect2(bounds[0] * pixels_per_unit, (bounds[1] - bounds[0]) * pixels_per_unit)
	for landmark in landmarks:
		var position: Vector2 = landmark.position * pixels_per_unit
		var size: Vector2 = landmark.size * pixels_per_unit
		content_rect = content_rect.merge(Rect2(position - size * 0.5, size))
		var baseline := position + Vector2(-size.x * 0.42, -size.y * 0.28)
		var text_size := font.get_string_size(landmark.name, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE)
		var text_rect := Rect2(baseline - Vector2(0, font.get_ascent(FONT_SIZE)), Vector2(text_size.x, font.get_height(FONT_SIZE)))
		content_rect = content_rect.merge(text_rect)
	for wall in geometry.get("walls", []):
		for i in range(wall.size() - 1):
			wall_segments.append(wall[i] * pixels_per_unit)
			wall_segments.append(wall[i + 1] * pixels_per_unit)
	var floor: PackedVector2Array = geometry.get("floor", PackedVector2Array())
	if not floor.is_empty():
		var vertices := PackedVector3Array()
		vertices.resize(floor.size())
		for i in floor.size():
			vertices[i] = Vector3(floor[i].x * pixels_per_unit, floor[i].y * pixels_per_unit, 0.0)
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		floor_mesh = ArrayMesh.new()
		floor_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)


func get_content_rect() -> Rect2:
	return content_rect


func _draw() -> void:
	for landmark in landmarks:
		var position: Vector2 = landmark.position * pixels_per_unit
		var half_size: Vector2 = landmark.size * pixels_per_unit * 0.5
		draw_rect(Rect2(position - half_size, half_size * 2.0), LANDMARK_BACKGROUND)
	if floor_mesh != null:
		draw_mesh(floor_mesh, null, Transform2D.IDENTITY, FLOOR)
	for landmark in landmarks:
		var position: Vector2 = landmark.position * pixels_per_unit
		var size: Vector2 = landmark.size * pixels_per_unit
		draw_rect(Rect2(position - size * 0.5, size), LANDMARK_BORDER, false, 2.0)
		draw_string(font, position + Vector2(-size.x * 0.42, -size.y * 0.28), landmark.name, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, INK)
	if not wall_segments.is_empty():
		draw_multiline(wall_segments, WALL, 2.0, true)
