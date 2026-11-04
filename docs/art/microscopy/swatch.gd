extends SceneTree

class Sheet extends Node2D:
	const CellMesh = preload("res://world_cell_mesh.gd")
	const RBC = preload("res://assets/cells/erythrocyte.png")
	const WBC = preload("res://assets/cells/neutrophil.png")
	var drawings := []
	func _ready() -> void:
		for row in 2:
			for col in 3:
				var points := PackedVector2Array()
				for i in 12:
					var p := Vector2.from_angle(TAU * i / 12) * (65 if row == 0 else 90)
					if col == 1: p *= Vector2(1.35, 0.5)
					if col == 2:
						p.x += 20 * sin(p.y / 40)
						p = p.rotated(0.5)
					points.append(p + Vector2(230 + col * 370, 260 + row * 310))
				var drawing = CellMesh.new()
				drawing.update(points, 1.0, Color.WHITE, Color.WHITE, true, true)
				drawings.append(drawing)
	func _draw() -> void:
		draw_rect(Rect2(0, 0, 1280, 800), Color("eee9e1"))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(55, 65), "Cell texture sample / production mesh", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, Color("51484c"))
		for col in 3:
			draw_string(font, Vector2(145 + col * 370, 125), ["Rest shape", "Compressed", "Bent / rotated"][col], HORIZONTAL_ALIGNMENT_LEFT, -1, 22, Color("61565e"))
		for i in drawings.size():
			var drawing = drawings[i]
			draw_mesh(drawing.mesh, RBC if i < 3 else WBC)
			draw_polyline(drawing.closed_contour, Color("bc8c86") if i < 3 else Color("a79aaa"), 1.3, true)
		draw_string(font, Vector2(55, 750), "Synthetic deformation examples; vessel and tissue art not included.", HORIZONTAL_ALIGNMENT_LEFT, -1, 19, Color("61565e"))

func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	root.add_child(Sheet.new())
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/swatch.png")
	quit()
