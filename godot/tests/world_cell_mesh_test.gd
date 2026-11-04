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
	_check_textured_deformation()
	_check_nucleus_display()
	_check_stable_appearance()
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


func _check_textured_deformation() -> void:
	var drawing = CellMesh.new()
	var cell := PackedVector2Array()
	for i in 12:
		cell.append(Vector2.from_angle(TAU * i / 12) * 20.0)
	var original := cell.duplicate()
	assert(drawing.update(cell, 1.0, Color.WHITE, Color.WHITE, true, true))
	assert(cell == original)
	assert(drawing.closed_contour.size() > cell.size() + 1)
	assert(drawing.closed_contour[0] == drawing.closed_contour[-1])
	var arrays: Array = drawing.mesh.surface_get_arrays(0)
	var uv: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
	var initial: PackedVector2Array = arrays[Mesh.ARRAY_VERTEX]
	assert(uv.size() == initial.size())
	for point in uv:
		assert(Rect2(0, 0, 1, 1).has_point(point))
	var deformation := Transform2D(Vector2(0, 0.45), Vector2(-1.5, 0), Vector2(90, 60))
	var squeezed: PackedVector2Array = deformation * cell
	assert(drawing.update(squeezed, 1.0, Color.WHITE, Color.WHITE, true, true))
	arrays = drawing.mesh.surface_get_arrays(0)
	assert(arrays[Mesh.ARRAY_TEX_UV] == uv)
	var moved: PackedVector2Array = arrays[Mesh.ARRAY_VERTEX]
	for i in initial.size():
		assert(moved[i].distance_to(deformation * initial[i]) < 0.0001)
	# Slot reuse may switch from textured cells back to ordinary two-layer cells.
	var square := PackedVector2Array([Vector2.ZERO, Vector2(2, 0), Vector2(2, 2), Vector2(0, 2)])
	assert(drawing.update(square, 1.0, Color.RED))
	_check_fill(drawing.mesh, square, 4.0)


func _check_nucleus_display() -> void:
	var drawing = CellMesh.new()
	var rest := PackedVector2Array()
	for i in 12:
		rest.append(Vector2.from_angle(TAU * i / 12) * 20.0)
	for squeeze in [1.0, 0.8, 0.35]:
		var shape := Transform2D(Vector2(1.2, 0), Vector2(0, squeeze), Vector2(70, 80)) * rest
		assert(drawing.update(shape, 1.0, Color.WHITE, Color.WHITE, true, true, 20.0))
		var pose: Transform2D = drawing.nucleus_transform(13.0)
		assert(absf(pose.x.length() - pose.y.length()) < 0.00001)
		if squeeze >= 0.8:
			assert(absf(pose.x.length() - 1.0) < 0.00001)
		else:
			assert(pose.x.length() < 0.6)
		# The same amount of cytoplasm texture always covers the same world length.
		var arrays: Array = drawing.mesh.surface_get_arrays(0)
		var uv: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
		var points: PackedVector2Array = arrays[Mesh.ARRAY_VERTEX]
		assert(absf(uv[0].distance_to(uv[10]) * 40.0 - points[0].distance_to(points[10])) < 0.0001)


func _check_stable_appearance() -> void:
	var drawing = CellMesh.new()
	var cell := PackedVector2Array()
	for i in 12:
		cell.append(Vector2.from_angle(TAU * i / 12) * 20.0)
	assert(drawing.update(cell, 1.0, Color.WHITE, Color.WHITE, true, true, 20.0))
	drawing.configure_appearance(82, 3)
	var original_pose: Transform2D = drawing.nucleus_transform(13.0)
	var original_variant: int = drawing.nucleus_variant
	drawing.configure_appearance(82, 3)
	assert(drawing.nucleus_transform(13.0).is_equal_approx(original_pose))
	drawing.configure_appearance(83, 3)
	assert(not drawing.nucleus_transform(13.0).is_equal_approx(original_pose))
	# Recreated display slots and returning IDs must recover the same appearance.
	drawing.configure_appearance(82, 3)
	assert(drawing.nucleus_transform(13.0).is_equal_approx(original_pose))
	var recreated = CellMesh.new()
	assert(recreated.update(cell, 1.0, Color.WHITE, Color.WHITE, true, true, 20.0))
	recreated.configure_appearance(82, 3)
	assert(recreated.nucleus_variant == original_variant)
	assert(recreated.nucleus_transform(13.0).is_equal_approx(original_pose))
