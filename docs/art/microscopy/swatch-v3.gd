extends SceneTree

class Sheet extends Node2D:
	const CellMesh = preload("res://world_cell_mesh.gd")
	const CYTO = preload("res://assets/cells/neutrophil-cytoplasm-v3.png")
	const NUCLEI = [preload("res://assets/cells/neutrophil-nucleus.png"), preload("res://assets/cells/neutrophil-nucleus-b.png"), preload("res://assets/cells/neutrophil-nucleus-c.png")]
	var drawings := []
	func _ready() -> void:
		texture_repeat = CanvasItem.TEXTURE_REPEAT_ENABLED
		for row in 2:
			for col in 3:
				var points := PackedVector2Array()
				for i in 12:
					var p := Vector2.from_angle(TAU * i / 12) * 95
					if row == 1: p *= Vector2(1.2, 0.8)
					points.append(p + Vector2(235 + col * 400, 260 + row * 310))
				var drawing = CellMesh.new()
				drawing.update(points, 1.0, Color.WHITE, Color.WHITE, true, true, 95.0)
				drawing.configure_appearance(82 + col, NUCLEI.size())
				drawings.append(drawing)
	func _draw() -> void:
		draw_rect(Rect2(0, 0, 1280, 800), Color("eee9e1"))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(55, 65), "Birth variations / stronger cytoplasm texture", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, Color("51484c"))
		for col in 3:
			draw_string(font, Vector2(155 + col * 400, 125), "Cell ID %d" % (82 + col), HORIZONTAL_ALIGNMENT_LEFT, -1, 22, Color("61565e"))
		for i in drawings.size():
			var drawing = drawings[i]
			draw_mesh(drawing.mesh, CYTO)
			var pose: Transform2D = drawing.nucleus_transform(83.6)
			draw_set_transform_matrix(pose)
			draw_texture_rect(NUCLEI[drawing.nucleus_variant], Rect2(-83.6, -83.6, 167.2, 167.2), false)
			draw_set_transform_matrix(Transform2D.IDENTITY)
			draw_polyline(drawing.closed_contour, Color("a79aaa"), 1.3, true)
		draw_string(font, Vector2(55, 750), "Same IDs in both rows: fixed arrangement and rotation; lower row compressed.", HORIZONTAL_ALIGNMENT_LEFT, -1, 20, Color("61565e"))

func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	root.add_child(Sheet.new())
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/swatch-v3.png")
	quit()
