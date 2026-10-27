#[cfg(test)]
mod geometry_query_audit {
    use super::{geometry_reference, graph};
    use crate::map::MapDefinition;
    use rapier2d::prelude::Vector;

    #[test]
    fn existing_velocity_field_is_preserved_across_world_and_boundaries() {
        let map = MapDefinition::small_world();
        let mut queries = 0;
        let mut max_error = 0.0_f32;
        for pressure in [0.0, 12.0, -3.0] {
            let flow = graph::GraphNetwork::new(&map, 0.05).solve(pressure);
            let original = geometry_reference::GraphNetwork::new(&map, 0.05).solve(pressure);
            for y in -40..=64 {
                for x in -72..=100 {
                    let position = Vector::new(x as f32 * 0.5, y as f32 * 0.5);
                    let actual = graph::sample_velocity(position, &map, &flow);
                    let expected = geometry_reference::sample_velocity(position, &map, &original);
                    max_error = max_error.max((actual - expected).length());
                    assert_eq!(
                        actual.x.to_bits(),
                        expected.x.to_bits(),
                        "x at {position:?}"
                    );
                    assert_eq!(
                        actual.y.to_bits(),
                        expected.y.to_bits(),
                        "y at {position:?}"
                    );
                    queries += 1;
                }
            }
        }
        println!("GEOMETRY_AUDIT queries={queries} max_velocity_error={max_error:.17e}");
    }
}
