//! Minimal SVG geometry shared by the two map diagnostics.
use simulation::map::MapDefinition;
use std::fmt::Write;

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn write_geometry(svg: &mut String, map: &MapDefinition, edge_title: impl Fn(usize) -> String) {
    for chamber in &map.chambers {
        let points = chamber
            .boundary
            .iter()
            .map(|p| format!("{},{}", p[0], p[1]))
            .collect::<Vec<_>>()
            .join(" ");
        write!(svg, r##"<polygon points="{points}" fill="#b65567"/>"##).unwrap();
    }
    for (i, edge) in map.edges.iter().enumerate() {
        let color = match edge.half_width as u32 {
            12 => "#f4ac75",
            6 => "#70c4b0",
            3 => "#78aef1",
            1 => "#d9cdf2",
            _ => "#b65567",
        };
        let points = edge
            .centerline
            .iter()
            .map(|p| format!("{},{}", p[0], p[1]))
            .collect::<Vec<_>>()
            .join(" ");
        write!(svg, r##"<polyline points="{points}" fill="none" stroke="{color}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"><title>{}</title></polyline>"##, edge.half_width * 2., escape(&edge_title(i))).unwrap();
    }
}
