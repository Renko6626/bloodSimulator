extends SceneTree
func _initialize() -> void:
	var mesh := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = PackedVector2Array([Vector2.ZERO, Vector2(1,0), Vector2(0,1)])
	arrays[Mesh.ARRAY_COLOR] = PackedColorArray([Color.RED, Color.RED, Color.RED])
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0,1,2])
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, Mesh.ARRAY_FLAG_USE_DYNAMIC_UPDATE)
	var format := mesh.surface_get_format(0)
	print("MESH_API ", format, " strides ",RenderingServer.mesh_surface_get_format_vertex_stride(format, 3)," ",RenderingServer.mesh_surface_get_format_index_stride(format,3))
	mesh.surface_update_vertex_region(0,0,PackedVector2Array([Vector2(2,0),Vector2(3,0),Vector2(2,1)]).to_byte_array())
	print("AFTER ", mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX])
	quit()
