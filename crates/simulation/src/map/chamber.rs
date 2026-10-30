use rapier2d::prelude::Vector;

/// A simple closed chamber outline, without holes. Either winding is accepted.
/// Callers supply a non-self-intersecting polygon; construction does not repair it.
#[derive(Clone, Debug)]
pub struct ChamberGeometry {
    pub name: String,
    pub boundary: Vec<[f32; 2]>,
}

impl ChamberGeometry {
    pub fn new(name: impl Into<String>, mut boundary: Vec<[f32; 2]>) -> Self {
        if boundary.len() > 1 && boundary.first() == boundary.last() {
            boundary.pop();
        }
        assert!(boundary.len() >= 3, "chamber needs at least three vertices");
        assert!(
            boundary.iter().flatten().all(|v| v.is_finite()),
            "chamber vertices must be finite"
        );
        let mut twice_area = 0.0_f64;
        for (a, b) in boundary.iter().zip(boundary.iter().cycle().skip(1)) {
            assert!(a != b, "chamber edges must have nonzero length");
            twice_area += a[0] as f64 * b[1] as f64 - b[0] as f64 * a[1] as f64;
        }
        assert!(twice_area.abs() > 0.0, "chamber must have nonzero area");
        Self {
            name: name.into(),
            boundary,
        }
    }

    /// Derived from the outline, so edits to the public boundary remain visible.
    pub fn bounds(&self) -> [[f32; 2]; 2] {
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in &self.boundary {
            for axis in 0..2 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        [min, max]
    }

    /// Negative inside, zero on an edge, positive outside (including concave notches).
    pub fn signed_distance(&self, position: Vector) -> f32 {
        let mut inside = false;
        let mut distance_squared = f32::INFINITY;
        for (a, b) in self
            .boundary
            .iter()
            .zip(self.boundary.iter().cycle().skip(1))
        {
            let start = Vector::new(a[0], a[1]);
            let delta = Vector::new(b[0], b[1]) - start;
            let t = ((position - start).dot(delta) / delta.length_squared()).clamp(0.0, 1.0);
            distance_squared =
                distance_squared.min((position - (start + delta * t)).length_squared());
            if (a[1] > position.y) != (b[1] > position.y)
                && position.x < (b[0] - a[0]) * (position.y - a[1]) / (b[1] - a[1]) + a[0]
            {
                inside = !inside;
            }
        }
        let distance = distance_squared.sqrt();
        if inside { -distance } else { distance }
    }
}
