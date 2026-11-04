extends Node2D
## Static drawing commands used once by the background viewport.

const TISSUE := preload("res://assets/environment/tissue.png")
const LUMEN := preload("res://assets/environment/lumen.png")
const LUMEN_TILE_PIXELS := 512.0
const WALL_FADE_PIXELS := 8.0
const ENDOTHELIUM := preload("res://assets/environment/endothelium.png")
const TISSUE_TILE_PIXELS := 768.0
const WALL_TILE_PIXELS := 192.0
# Slight rose is artistic blood-scene tint; normal plasma itself is pale yellow.
const FLOOR := Color("f2e2da")
const LANDMARK_BACKGROUND := Color("e4ded3")
const LANDMARK_BORDER := Color("b7aea5")
const INK := Color("746c69")
const WALL := Color("b6a4a5")
const FONT_SIZE := 20

var wall_fade_mesh: ArrayMesh
var wall_mesh: ArrayMesh
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
	texture_repeat = CanvasItem.TEXTURE_REPEAT_ENABLED
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
	_build_wall_mesh()
	var floor: PackedVector2Array = geometry.get("floor", PackedVector2Array())
	if not floor.is_empty():
		var vertices := PackedVector3Array()
		vertices.resize(floor.size())
		for i in floor.size():
			vertices[i] = Vector3(floor[i].x * pixels_per_unit, floor[i].y * pixels_per_unit, 0.0)
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		var uv := PackedVector2Array()
		for point in floor:
			uv.append(point * pixels_per_unit / LUMEN_TILE_PIXELS)
		arrays[Mesh.ARRAY_TEX_UV] = uv
		floor_mesh = ArrayMesh.new()
		floor_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)


func get_content_rect() -> Rect2:
	return content_rect


func _draw() -> void:
	draw_tissue(self, content_rect.grow(8.0))
	for landmark in landmarks:
		var position: Vector2 = landmark.position * pixels_per_unit
		var half_size: Vector2 = landmark.size * pixels_per_unit * 0.5
		draw_rect(Rect2(position - half_size, half_size * 2.0), LANDMARK_BACKGROUND)
	if floor_mesh != null:
		draw_mesh(floor_mesh, null, Transform2D.IDENTITY, FLOOR)
		draw_mesh(floor_mesh, LUMEN, Transform2D.IDENTITY, Color(1, 1, 1, 0.85))
	if wall_fade_mesh != null:
		draw_mesh(wall_fade_mesh, null)
	for landmark in landmarks:
		var position: Vector2 = landmark.position * pixels_per_unit
		var size: Vector2 = landmark.size * pixels_per_unit
		draw_rect(Rect2(position - size * 0.5, size), LANDMARK_BORDER, false, 2.0)
		draw_string(font, position + Vector2(-size.x * 0.42, -size.y * 0.28), landmark.name, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, INK)
	if not wall_segments.is_empty():
		draw_multiline(wall_segments, WALL, 8.0, true)
		if wall_mesh != null:
			draw_mesh(wall_mesh, ENDOTHELIUM)


static func draw_tissue(canvas: CanvasItem, rect: Rect2) -> void:
	# Identical world-space UVs in cached chunks and the uncovered-area backdrop.
	var points := PackedVector2Array([rect.position, Vector2(rect.end.x, rect.position.y), rect.end, Vector2(rect.position.x, rect.end.y)])
	var uv := PackedVector2Array()
	for point in points:
		uv.append(point / TISSUE_TILE_PIXELS)
	canvas.draw_polygon(points, PackedColorArray([Color.WHITE]), uv, TISSUE)


func _build_wall_mesh() -> void:
	var fade_vertices := PackedVector3Array()
	var fade_colors := PackedColorArray()
	var vertices := PackedVector3Array()
	var uv := PackedVector2Array()
	for i in range(0, wall_segments.size(), 2):
		var a := wall_segments[i]
		var b := wall_segments[i + 1]
		var direction := (b - a).normalized()
		if direction == Vector2.ZERO:
			continue
		# Fade stays inside the existing 8-pixel cache padding. This is only
		# a subtle color transition around the wall, never a collision offset.
		for side in [-1.0, 1.0]:
			var offset: Vector2 = direction.orthogonal() * WALL_FADE_PIXELS * side
			var fade_points := PackedVector2Array([a, b, b + offset, a + offset])
			for corner in [0, 1, 2, 0, 2, 3]:
				fade_vertices.append(Vector3(fade_points[corner].x, fade_points[corner].y, 0))
				fade_colors.append(Color(0.62, 0.38, 0.36, 0.22 if corner < 2 else 0.0))
		var normal := direction.orthogonal() * 3.0
		var corners := PackedVector2Array([a - normal, b - normal, b + normal, a + normal])
		var start := a.dot(direction) / WALL_TILE_PIXELS
		var end := b.dot(direction) / WALL_TILE_PIXELS
		# Sample the endothelial row in the middle of the source square.
		var coords := PackedVector2Array([Vector2(start, 0.45), Vector2(end, 0.45), Vector2(end, 0.55), Vector2(start, 0.55)])
		for corner in [0, 1, 2, 0, 2, 3]:
			vertices.append(Vector3(corners[corner].x, corners[corner].y, 0))
			uv.append(coords[corner])
	if vertices.is_empty():
		return
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_TEX_UV] = uv
	wall_mesh = ArrayMesh.new()
	wall_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)

	var fade_arrays := []
	fade_arrays.resize(Mesh.ARRAY_MAX)
	fade_arrays[Mesh.ARRAY_VERTEX] = fade_vertices
	fade_arrays[Mesh.ARRAY_COLOR] = fade_colors
	wall_fade_mesh = ArrayMesh.new()
	wall_fade_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, fade_arrays)
