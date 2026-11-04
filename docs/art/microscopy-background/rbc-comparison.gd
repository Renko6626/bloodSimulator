extends SceneTree
class Sheet extends Node2D:
	const CellMesh = preload("res://world_cell_mesh.gd")
	const OLD = preload("res://assets/cells/erythrocyte.png")
	const NEW = preload("res://assets/cells/erythrocyte-v2.png")
	var drawings := []
	func _ready() -> void:
		for row in 2:
			for col in 2:
				var points := PackedVector2Array()
				for i in 12:
					points.append(Vector2.from_angle(TAU * i / 12) * (32 if row == 0 else 110) + Vector2(340 + col * 600, 240 + row * 300))
				var mesh = CellMesh.new()
				mesh.update(points, 1, Color.WHITE, Color.WHITE, true, true)
				drawings.append(mesh)
	func _draw() -> void:
		draw_rect(Rect2(0, 0, 1280, 800), Color("f2e2da"))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(55, 65), "Red-cell material / actual renderer", HORIZONTAL_ALIGNMENT_LEFT, -1, 28, Color("51484c"))
		draw_string(font, Vector2(260, 130), "Previous", HORIZONTAL_ALIGNMENT_LEFT, -1, 25, Color("61565e"))
		draw_string(font, Vector2(855, 130), "New sample", HORIZONTAL_ALIGNMENT_LEFT, -1, 25, Color("61565e"))
		for i in drawings.size():
			var mesh = drawings[i]
			draw_mesh(mesh.mesh, OLD if i % 2 == 0 else NEW)
			draw_polyline(mesh.closed_contour, Color("cb9186"), 1.0, true)
		draw_string(font, Vector2(55, 750), "Top: gameplay scale. Bottom: enlarged material comparison.", HORIZONTAL_ALIGNMENT_LEFT, -1, 20, Color("61565e"))
func _initialize() -> void: _run.call_deferred()
func _run() -> void:
	root.add_child(Sheet.new())
	for i in 8: await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png("/tmp/sunyunbo/cell-texture/rbc-comparison.png")
	quit()
