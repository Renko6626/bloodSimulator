extends RefCounted
## Reusable fill mesh. Display smoothing and UVs never alter simulation nodes.

const INNER_COLOR := Color("6f293e")
const INNER_SCALE := 0.48
const CORNER_FRACTION := 0.35
const CORNER_STEPS := 4

var mesh := ArrayMesh.new()
var closed_contour := PackedVector2Array()
var vertices := PackedVector2Array()
var combined_indices := PackedInt32Array()
var previous_triangles := PackedInt32Array()
var previous_count := 0
var previous_color := Color.TRANSPARENT
var previous_inner_color := Color.TRANSPARENT
var previous_textured := false
var previous_smooth := false
var previous_rigid_texture := false
var cell_pose := Transform2D.IDENTITY
var appearance_id := -1
var appearance_variant_count := 0
var nucleus_variant := 0
var nucleus_angle := 0.0


func update(cell: PackedVector2Array, pixels_per_unit: float, color: Color, inner_color: Color = INNER_COLOR, smooth: bool = false, textured: bool = false, rigid_texture_radius: float = 0.0) -> bool:
	if cell.size() < 3:
		closed_contour.clear()
		return false
	var transform := Transform2D(Vector2(pixels_per_unit, 0), Vector2(0, pixels_per_unit), Vector2.ZERO)
	var contour: PackedVector2Array = transform * cell
	if smooth:
		contour = rounded_contour(contour)
	var count := contour.size()
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
	var rigid_texture := textured and rigid_texture_radius > 0.0
	var orientation := Vector2.ZERO
	if rigid_texture:
		for i in cell.size():
			var reference := Vector2.from_angle(TAU * i / cell.size())
			var offset: Vector2 = cell[i] * pixels_per_unit - center
			orientation += Vector2(reference.dot(offset), reference.cross(offset))
	cell_pose = Transform2D(orientation.angle(), center)
	var layers := 1 if textured else 2
	vertices.resize(count * layers)
	closed_contour.resize(count + 1)
	for i in count:
		vertices[i] = contour[i]
		if not textured:
			vertices[count + i] = center + (contour[i] - center) * INNER_SCALE
		closed_contour[i] = contour[i]
	closed_contour[count] = contour[0]
	var rebuild := count != previous_count or color != previous_color or inner_color != previous_inner_color or textured != previous_textured or smooth != previous_smooth or rigid_texture or rigid_texture != previous_rigid_texture
	var changed_triangles := triangles != previous_triangles
	if rebuild or changed_triangles:
		combined_indices.resize(triangles.size() * layers)
		for i in triangles.size():
			combined_indices[i] = triangles[i]
			if not textured:
				combined_indices[triangles.size() + i] = triangles[i] + count
	if rebuild:
		var colors := PackedColorArray()
		colors.resize(count * layers)
		colors.fill(inner_color)
		for i in count:
			colors[i] = color
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_COLOR] = colors
		arrays[Mesh.ARRAY_INDEX] = combined_indices
		if textured:
			# Node indices follow material points, so texture rotates/deforms with them.
			# Recomputing UVs from a world-space AABB would make the surface swim.
			var uv := PackedVector2Array()
			for i in cell.size():
				uv.append(Vector2(0.5, 0.5) + Vector2.from_angle(TAU * i / cell.size()) * 0.48)
			if rigid_texture:
				# Constant-scale cytoplasm, clipped by the membrane. No texture dynamics.
				# Only the few neutrophils rebuild attributes as their display UVs change.
				uv.clear()
				var inverse := cell_pose.affine_inverse()
				for point in contour:
					uv.append(Vector2(0.5, 0.5) + (inverse * point) / (2.0 * rigid_texture_radius))
				arrays[Mesh.ARRAY_TEX_UV] = uv
			else:
				arrays[Mesh.ARRAY_TEX_UV] = rounded_contour(uv) if smooth else uv
		mesh.clear_surfaces()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, Mesh.ARRAY_FLAG_USE_DYNAMIC_UPDATE)
		previous_count = count
		previous_color = color
		previous_inner_color = inner_color
		previous_textured = textured
		previous_smooth = smooth
		previous_rigid_texture = rigid_texture
	else:
		mesh.surface_update_vertex_region(0, 0, vertices.to_byte_array())
		if changed_triangles:
			_update_indices()
	previous_triangles = triangles
	mesh.custom_aabb = AABB(Vector3(bounds.position.x, bounds.position.y, 0), Vector3(bounds.size.x, bounds.size.y, 0))
	return true


func configure_appearance(entity_id: int, variant_count: int) -> void:
	if appearance_id == entity_id and appearance_variant_count == variant_count:
		return
	# ID-derived appearance survives culling, hibernation and render-slot reuse.
	# A local RNG leaves the simulation and global random sequence untouched.
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(str(entity_id))
	nucleus_variant = rng.randi_range(0, maxi(variant_count - 1, 0))
	nucleus_angle = rng.randf_range(-0.6, 0.6)
	appearance_id = entity_id
	appearance_variant_count = variant_count


func nucleus_transform(radius: float) -> Transform2D:
	# Pure display fit: preserve proportions, shrink only if the centered envelope
	# touches the membrane. No nucleus body, force, velocity, or persistent state.
	var clearance := radius
	if closed_contour.size() < 4 or not Geometry2D.is_point_in_polygon(cell_pose.origin, closed_contour):
		clearance = 0.0
	else:
		for i in closed_contour.size() - 1:
			var closest := Geometry2D.get_closest_point_to_segment(cell_pose.origin, closed_contour[i], closed_contour[i + 1])
			clearance = minf(clearance, maxf(0.0, cell_pose.origin.distance_to(closest) - 0.5))
	var fit := clearance / radius if radius > 0.0 else 0.0
	return cell_pose * Transform2D(nucleus_angle, Vector2.ZERO).scaled_local(Vector2.ONE * fit)


static func rounded_contour(points: PackedVector2Array) -> PackedVector2Array:
	# Round each corner within its local control triangle; no outward spline overshoot.
	var result := PackedVector2Array()
	for i in points.size():
		var corner := points[i]
		var start := corner.lerp(points[(i - 1 + points.size()) % points.size()], CORNER_FRACTION)
		var end := corner.lerp(points[(i + 1) % points.size()], CORNER_FRACTION)
		for step in range(CORNER_STEPS + 1):
			var t := float(step) / CORNER_STEPS
			result.append(start.lerp(corner, t).lerp(corner.lerp(end, t), t))
	return result


func _update_indices() -> void:
	var stride := RenderingServer.mesh_surface_get_format_index_stride(mesh.surface_get_format(0), vertices.size())
	var bytes := combined_indices.to_byte_array()
	if stride == 2:
		bytes.resize(combined_indices.size() * 2)
		for i in combined_indices.size():
			bytes.encode_u16(i * 2, combined_indices[i])
	RenderingServer.mesh_surface_update_index_region(mesh.get_rid(), 0, 0, bytes)
