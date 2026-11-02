"""Annotate map_render SVG exports and convert with the installed librsvg/cairo."""
from pathlib import Path
import xml.etree.ElementTree as ET
import gi
import cairo

gi.require_version("Rsvg", "2.0")
from gi.repository import Rsvg

root = Path(__file__).parent
for name, x, y, size in [("overview", 120, 90, 14), ("heart-lung", 380, 80, 7)]:
    path = root / (name + ".svg")
    svg = path.read_text()
    label = "ACTUAL GEOMETRY / ZERO RUNTIME FLOW"
    if label not in svg:
        svg = svg.replace("</svg>", f'<text x="{x}" y="{y}" font-size="{size}" fill="white">{label}</text></svg>')
        path.write_text(svg)
    node = ET.parse(path).getroot()
    width, height = (int(float(node.attrib[k])) for k in ("width", "height"))
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, width, height)
    viewport = Rsvg.Rectangle()
    viewport.x = viewport.y = 0
    viewport.width, viewport.height = width, height
    Rsvg.Handle.new_from_file(str(path)).render_document(cairo.Context(surface), viewport)
    surface.write_to_png(str(root / (name + ".png")))
