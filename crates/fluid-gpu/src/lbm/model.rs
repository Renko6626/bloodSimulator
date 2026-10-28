#[cfg(test)]
use super::WindowSpec;
pub(super) const C: [[f32; 2]; 9] = [
    [0., 0.],
    [1., 0.],
    [0., 1.],
    [-1., 0.],
    [0., -1.],
    [1., 1.],
    [-1., 1.],
    [-1., -1.],
    [1., -1.],
];
const W: [f32; 9] = [
    4. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 36.,
    1. / 36.,
    1. / 36.,
    1. / 36.,
];
pub(super) fn equilibrium(rho: f32, u: [f32; 2]) -> [f32; 9] {
    std::array::from_fn(|q| {
        let cu = C[q][0] * u[0] + C[q][1] * u[1];
        W[q] * ((rho - 1.) + rho * (3. * cu + 4.5 * cu * cu - 1.5 * (u[0] * u[0] + u[1] * u[1])))
    })
}
pub(super) fn initial(rho: f32, u: [f32; 2], force: [f32; 2]) -> [f32; 9] {
    let mut populations = equilibrium(rho, u);
    for q in 0..9 {
        populations[q] -= 1.5 * W[q] * (C[q][0] * force[0] + C[q][1] * force[1]);
    }
    populations
}
#[cfg(test)]
pub(super) fn cell_position(spec: WindowSpec, x: u32, y: u32) -> [f32; 2] {
    let o = spec.world_bounds()[0];
    let h = spec.profile.cell_spacing();
    [o[0] + (x as f32 + 0.5) * h, o[1] + (y as f32 + 0.5) * h]
}
