use super::{WindowGeometry, WindowSide};
pub fn geometry_blob(n: usize, geometry: &WindowGeometry, periodic: bool) -> Vec<u8> {
    let mut bytes = b"LSQGv1\0\0".to_vec();
    bytes.extend((n as u32).to_le_bytes());
    bytes.extend(u32::from(periodic).to_le_bytes());
    bytes.extend(geometry.solid.iter().map(|s| u8::from(*s)));
    let mut sides = vec![0u8; n * n];
    for o in &geometry.openings {
        for c in o.cells.clone() {
            let (x, y, s) = match o.side {
                WindowSide::Left => (0, c as usize, 1),
                WindowSide::Right => (n - 1, c as usize, 2),
                WindowSide::Bottom => (c as usize, 0, 3),
                WindowSide::Top => (c as usize, n - 1, 4),
            };
            sides[y * n + x] = s;
        }
    }
    bytes.extend(sides);
    bytes
}
