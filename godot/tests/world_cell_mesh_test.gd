extends SceneTree
## Explicit GL diagnostic: surface reads must observe GPU region updates.

const CellMesh = preload("res://world_cell_mesh.gd")


func _initialize() -> void:
	create_timer(5.0).timeout.connect(func(): quit(1))
	_run.call_deferred()


func _run() -> void:
	var drawing = CellMesh.new()
	var concave := PackedVector2Array([Vector2(0, 0), Vector2(2, 0), Vector2(2, 1), Vector2(1, 1), Vector2(1, 2), Vector2(0, 2)])
	assert(drawing.update(concave, 1.0, Color.RED))
	_check_fill(drawing.mesh, concave, 3.0)
	var original_mesh: ArrayMesh = drawing.mesh
	var moved: PackedVector2Array = Transform2D(0.0, Vector2(4, -3)) * concave
	assert(drawing.update(moved, 1.0, Color.RED))
	assert(drawing.mesh == original_mesh)
	_check_fill(drawing.mesh, moved, 3.0)
	assert(drawing.closed_contour[0] == drawing.closed_contour[-1])
	var changed := PackedVector2Array([Vector2(0, 0), Vector2(3, 0), Vector2(3, 3), Vector2(2, 3), Vector2(2, 1), Vector2(0, 1)])
	assert(drawing.update(changed, 1.0, Color.RED))
	_check_fill(drawing.mesh, changed, 5.0)
	var square := PackedVector2Array([Vector2.ZERO, Vector2(2, 0), Vector2(2, 2), Vector2(0, 2)])
	assert(drawing.update(square, 1.0, Color.RED))
	_check_fill(drawing.mesh, square, 4.0)
	assert(not drawing.update(PackedVector2Array(), 1.0, Color.RED))
	assert(drawing.closed_contour.is_empty())
	print("CELL_MESH_TEST PASS: concavity, GPU vertex/index updates, node count and empty input")
	quit()


func _check_fill(mesh: ArrayMesh, polygon: PackedVector2Array, area: float) -> void:
	var arrays := mesh.surface_get_arrays(0)
	var points: PackedVector2Array = arrays[Mesh.ARRAY_VERTEX]
	var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	assert(points.size() == polygon.size() * 2)
	var outer_area := 0.0
	var inner_area := 0.0
	for i in range(0, indices.size(), 3):
		var a := points[indices[i]]
		var b := points[indices[i + 1]]
		var c := points[indices[i + 2]]
		var triangle_area := absf((b - a).cross(c - a)) * 0.5
		if indices[i] < polygon.size():
			assert(Geometry2D.is_point_in_polygon((a + b + c) / 3.0, polygon))
			outer_area += triangle_area
		else:
			inner_area += triangle_area
	assert(absf(outer_area - area) < 0.00001)
	assert(absf(inner_area - area * 0.48 * 0.48) < 0.00001)
	for i in polygon.size():
		assert(points[i].is_equal_approx(polygon[i]))
