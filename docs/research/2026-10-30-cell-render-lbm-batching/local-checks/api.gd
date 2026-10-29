extends SceneTree
func _initialize() -> void:
	for method in RenderingServer.get_method_list():
		var name: String = method.name
		if name.begins_with("mesh_surface_") and ("update" in name or "stride" in name):
			print(name, " ", method.args)
	print("FLAGS ", Mesh.ARRAY_FLAG_USE_2D_VERTICES, " ", Mesh.ARRAY_FLAG_USE_DYNAMIC_UPDATE)
	quit()
