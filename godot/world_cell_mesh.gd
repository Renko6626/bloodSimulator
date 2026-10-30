extends RefCounted
## One reusable fill mesh per snapshot slot; the World keeps the AA outline.

const INNER_COLOR := Color("6f293e")
const INNER_SCALE := 0.48

var mesh := ArrayMesh.new()
var closed_contour := PackedVector2Array()
var vertices := PackedVector2Array()
var combined_indices := PackedInt32Array()
var previous_triangles := PackedInt32Array()
var previous_count := 0
var previous_color := Color.TRANSPARENT
var previous_inner_color := Color.TRANSPARENT


func update(cell: PackedVector2Array, pixels_per_unit: float, color: Color, inner_color: Color = INNER_COLOR) -> bool:
	var count := cell.size()
	if count < 3:
		closed_contour.clear()
		return false
	var transform := Transform2D(Vector2(pixels_per_unit, 0), Vector2(0, pixels_per_unit), Vector2.ZERO)
	var contour: PackedVector2Array = transform * cell
	var triangles := Geometry2D.triangulate_polygon(contour)
	if triangles.is_empty():
		closed_contour.clear()
		return false
	var center := Vector2.ZERO
	var bounds := Rect2(contour[0], Vector2.ZERO)
	for point in contour:
		center += point
		bounds = bounds.expand(point)
	center /= count
	vertices.resize(count * 2)
	closed_contour.resize(count + 1)
	for i in count:
		vertices[i] = contour[i]
		vertices[count + i] = center + (contour[i] - center) * INNER_SCALE
		closed_contour[i] = contour[i]
	closed_contour[count] = contour[0]
	var rebuild := count != previous_count or color != previous_color or inner_color != previous_inner_color
	var changed_triangles := triangles != previous_triangles
	if rebuild or changed_triangles:
		combined_indices.resize(triangles.size() * 2)
		for i in triangles.size():
			combined_indices[i] = triangles[i]
			combined_indices[triangles.size() + i] = triangles[i] + count
	if rebuild:
		var colors := PackedColorArray()
		colors.resize(count * 2)
		colors.fill(inner_color)
		for i in count:
			colors[i] = color
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_COLOR] = colors
		arrays[Mesh.ARRAY_INDEX] = combined_indices
		mesh.clear_surfaces()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, Mesh.ARRAY_FLAG_USE_DYNAMIC_UPDATE)
		previous_count = count
		previous_color = color
		previous_inner_color = inner_color
	else:
		mesh.surface_update_vertex_region(0, 0, vertices.to_byte_array())
		if changed_triangles:
			_update_indices()
	previous_triangles = triangles
	mesh.custom_aabb = AABB(Vector3(bounds.position.x, bounds.position.y, 0), Vector3(bounds.size.x, bounds.size.y, 0))
	return true


func _update_indices() -> void:
	var stride := RenderingServer.mesh_surface_get_format_index_stride(mesh.surface_get_format(0), vertices.size())
	var bytes := combined_indices.to_byte_array()
	if stride == 2:
		bytes.resize(combined_indices.size() * 2)
		for i in combined_indices.size():
			bytes.encode_u16(i * 2, combined_indices[i])
	RenderingServer.mesh_surface_update_index_region(mesh.get_rid(), 0, 0, bytes)
