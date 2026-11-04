extends SceneTree

class Sheet extends Node2D:
	const CellMesh = preload("res://world_cell_mesh.gd")
	const CYTO = preload("res://assets/cells/neutrophil-cytoplasm.png")
	const NUCLEUS = preload("res://assets/cells/neutrophil-nucleus.png")
	var drawings := []
	func _ready() -> void:
		texture_repeat = CanvasItem.TEXTURE_REPEAT_ENABLED
		for col in 3:
			var points := PackedVector2Array()
			for i in 12:
				var p := Vector2.from_angle(TAU * i / 12) * 110
				if col == 1: p *= Vector2(1.2, 0.8)
				if col == 2: p *= Vector2(1.4, 0.35)
				points.append(p + Vector2(235 + col * 400, 360))
			var drawing = CellMesh.new()
			drawing.update(points, 1.0, Color.WHITE, Color.WHITE, true, true, 110.0)
			drawings.append(drawing)
	func _draw() -> void:
		draw_rect(Rect2(0, 0, 1280, 800), Color("eee9e1"))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(55, 70), "Separate nucleus / fixed-scale cytoplasm", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, Color("51484c"))
		for col in 3:
			draw_string(font, Vector2(115 + col * 400, 180), ["Rest shape", "Moderate compression", "Tight compression"][col], HORIZONTAL_ALIGNMENT_LEFT, -1, 22, Color("61565e"))
			var drawing = drawings[col]
			draw_mesh(drawing.mesh, CYTO)
			var pose: Transform2D = drawing.nucleus_transform(71.5)
			draw_set_transform_matrix(pose)
			draw_texture_rect(NUCLEUS, Rect2(-71.5, -71.5, 143, 143), false)
			draw_set_transform_matrix(Transform2D.IDENTITY)
			draw_polyline(drawing.closed_contour, Color("a79aaa"), 1.3, true)
			draw_string(font, Vector2(130 + col * 400, 560), "Nucleus size: %d%%" % roundi(pose.x.length() * 100), HORIZONTAL_ALIGNMENT_LEFT, -1, 23, Color("61565e"))
		draw_string(font, Vector2(55, 700), "Display-only fit. No nuclear physics; no texture simulation.", HORIZONTAL_ALIGNMENT_LEFT, -1, 20, Color("61565e"))

func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	root.add_child(Sheet.new())
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/swatch-v2.png")
	quit()
