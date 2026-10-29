extends Node2D
## One map-sized GPU texture. Dynamic redraws never update its source viewport.

const Painter = preload("res://world_background_painter.gd")
const PADDING := Vector2(8, 8)
const MAX_SIZE := 4096

var bake_viewport: SubViewport
var sprite: Sprite2D


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
