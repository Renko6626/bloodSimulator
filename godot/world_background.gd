extends Node2D
## One map-sized GPU texture. Dynamic redraws never update its source viewport.

const Painter = preload("res://world_background_painter.gd")
const PADDING := Vector2(8, 8)
const MAX_SIZE := 4096

var bake_viewport: SubViewport
var sprite: Sprite2D
var chunks: Dictionary = {}


func clear_chunks() -> void:
	for node in chunks.values():
		node.queue_free()
	chunks.clear()


func apply_chunk_delta(delta: Dictionary, pixels_per_unit: float, font: Font) -> String:
	if delta.reset:
		clear_chunks()
	for id in delta.removed:
		if chunks.has(id):
			chunks[id].queue_free()
			chunks.erase(id)
	if DisplayServer.get_name() == "headless" or not RenderingServer.is_render_loop_enabled():
		return ""
	for chunk in delta.added:
		var painter := Painter.new()
		painter.configure_chunk(chunk, delta.landmarks, pixels_per_unit, font)
		if not painter.has_content():
			painter.free()
			continue
		var bounds: PackedVector2Array = chunk.bounds
		var origin: Vector2 = bounds[0] * pixels_per_unit
		var core_size: Vector2 = (bounds[1] - bounds[0]) * pixels_per_unit
		var root := Node2D.new()
		root.name = "Chunk_%s_%s" % [chunk.id.x, chunk.id.y]
		var viewport := SubViewport.new()
		viewport.world_2d = World2D.new()
		viewport.size = Vector2i(core_size + PADDING * 2.0)
		viewport.disable_3d = true
		viewport.use_hdr_2d = false
		viewport.msaa_2d = Viewport.MSAA_DISABLED
		viewport.transparent_bg = false
		viewport.render_target_clear_mode = SubViewport.CLEAR_MODE_ONCE
		viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
		painter.position = -origin + PADDING
		add_child(root)
		root.add_child(viewport)
		viewport.add_child(painter)
		var tile := Sprite2D.new()
		tile.centered = false
		tile.position = origin
		tile.region_enabled = true
		tile.region_rect = Rect2(PADDING, core_size)
		tile.region_filter_clip_enabled = false
		tile.texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR
		root.add_child(tile)
		chunks[chunk.id] = root
		_bind_chunk_texture.call_deferred(viewport, tile)
	return ""


func _bind_chunk_texture(viewport: SubViewport, tile: Sprite2D) -> void:
	tile.texture = viewport.get_texture()


func configure(geometry: Dictionary, pixels_per_unit: float, font: Font) -> String:
	if DisplayServer.get_name() == "headless" or not RenderingServer.is_render_loop_enabled():
		return ""
	var painter := Painter.new()
	painter.configure(geometry, pixels_per_unit, font)
	var content := painter.get_content_rect()
	var origin := content.position.floor() - PADDING
	var end := content.end.ceil() + PADDING
	var texture_size := Vector2i(end - origin)
	if texture_size.x > MAX_SIZE or texture_size.y > MAX_SIZE:
		painter.free()
		return "Static background size %s exceeds %d x %d" % [texture_size, MAX_SIZE, MAX_SIZE]
	bake_viewport = SubViewport.new()
	bake_viewport.name = "BakeViewport"
	bake_viewport.world_2d = World2D.new()
	bake_viewport.size = texture_size
	bake_viewport.disable_3d = true
	bake_viewport.use_hdr_2d = false
	bake_viewport.msaa_2d = Viewport.MSAA_DISABLED
	bake_viewport.transparent_bg = false
	bake_viewport.render_target_clear_mode = SubViewport.CLEAR_MODE_ONCE
	bake_viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
	painter.position = -origin
	add_child(bake_viewport)
	bake_viewport.add_child(painter)
	sprite = Sprite2D.new()
	sprite.name = "BackgroundSprite"
	sprite.centered = false
	sprite.position = origin
	sprite.texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR
	add_child(sprite)
	_bind_texture.call_deferred()
	return ""


func _bind_texture() -> void:
	sprite.texture = bake_viewport.get_texture()
