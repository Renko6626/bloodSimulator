extends SceneTree

class Sheet extends Node2D:
	const CellMesh = preload("res://world_cell_mesh.gd")
	const CYTO = preload("res://assets/cells/neutrophil-cytoplasm-v3.png")
	const NUCLEI = [preload("res://assets/cells/neutrophil-nucleus.png"), preload("res://assets/cells/neutrophil-nucleus-b.png"), preload("res://assets/cells/neutrophil-nucleus-c.png")]
	var drawings := []
	var ids := []
	func _ready() -> void:
		texture_repeat = CanvasItem.TEXTURE_REPEAT_ENABLED
		var sim := BloodLab.new()
		sim.use_combat_preview()
		var snapshot: Dictionary = sim.snapshot()
		var source := []
		for i in snapshot.cells.size():
			if snapshot.cell_status[i].species == "neutrophil":
				source.append(snapshot.cells[i])
				ids.append(snapshot.cell_status[i].id)
		for row in 2:
			for col in mini(3, source.size()):
				var cell: PackedVector2Array = source[col]
				var center := Vector2.ZERO
				for p in cell: center += p
				center /= cell.size()
				var points := PackedVector2Array()
				for i in cell.size():
					var p: Vector2 = Vector2.from_angle(TAU * i / cell.size()) if row == 0 else cell[i] - center
					points.append(p * 95 + Vector2(235 + col * 400, 260 + row * 310))
				var drawing = CellMesh.new()
				drawing.update(points, 1.0, Color.WHITE, Color.WHITE, true, true, 95.0)
				drawing.configure_appearance(ids[col], NUCLEI.size())
				drawings.append(drawing)
	func _draw() -> void:
		draw_rect(Rect2(0, 0, 1280, 800), Color("eee9e1"))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(55, 65), "White-cell rest shapes / actual simulation snapshot", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, Color("51484c"))
		draw_string(font, Vector2(55, 135), "Previous circular reference", HORIZONTAL_ALIGNMENT_LEFT, -1, 21, Color("61565e"))
		draw_string(font, Vector2(55, 440), "New individual rest shapes", HORIZONTAL_ALIGNMENT_LEFT, -1, 21, Color("61565e"))
		for i in drawings.size():
			var drawing = drawings[i]
			draw_mesh(drawing.mesh, CYTO)
			draw_set_transform_matrix(drawing.nucleus_transform(83.6))
			draw_texture_rect(NUCLEI[drawing.nucleus_variant], Rect2(-83.6, -83.6, 167.2, 167.2), false)
			draw_set_transform_matrix(Transform2D.IDENTITY)
			draw_polyline(drawing.closed_contour, Color("a79aaa"), 1.3, true)
		draw_string(font, Vector2(55, 750), "Same area and node count. Lower outlines come from the new physical rest geometry.", HORIZONTAL_ALIGNMENT_LEFT, -1, 19, Color("61565e"))

func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	root.add_child(Sheet.new())
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/swatch-v4.png")
	quit()
