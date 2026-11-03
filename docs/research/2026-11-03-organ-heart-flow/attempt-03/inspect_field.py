"""Read-only inspection of the actual BFLOW001 output; never rewrites velocities."""
from pathlib import Path
import json
import struct
import sys
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

path = Path(sys.argv[1])
data = path.read_bytes()
assert data[:8] == b'BFLOW001'
revision = struct.unpack_from('<Q', data, 8)[0]
viscosity, pressure, max_scale, ox, oy, h = struct.unpack_from('<6f', data, 16)
nx, ny, count = struct.unpack_from('<3I', data, 40)
ports = [struct.unpack_from('<I3f', data, 52 + 16*i) for i in range(count)]
offset = 52 + count * 16
velocity = np.frombuffer(data, '<f4', nx * ny * 2, offset).reshape(ny, nx, 2)
fluid = np.frombuffer(data, 'u1', nx * ny, offset + nx * ny * 8).reshape(ny, nx).astype(bool)

def sample(points):
    q = (points - np.array([ox, oy])) / h
    assert (q >= 0).all() and (q[:, 0] <= nx-1).all() and (q[:, 1] <= ny-1).all()
    index = np.minimum(np.floor(q).astype(int), [nx-2, ny-2])
    t = q - index
    result = np.zeros_like(points)
    for dx, dy in [(0, 0), (1, 0), (0, 1), (1, 1)]:
        x, y = index[:, 0]+dx, index[:, 1]+dy
        w = (t[:, 0] if dx else 1-t[:, 0]) * (t[:, 1] if dy else 1-t[:, 1])
        result += velocity[y, x] * fluid[y, x, None] * w[:, None]
    return result

geometry = [
    ('systemic_in', [488., 656.], [0., 1.], 48., 5.),
    ('pulmonary_out', [528., 656.], [0., -1.], 48., 4.5),
    ('pulmonary_in', [562., 656.], [0., 1.], 48., 5.),
    ('systemic_out', [600., 656.], [0., -1.], 48., 5.),
]
metrics = {'source': path.name, 'boundary_trace_is_prescribed': True, 'interior_is_solver_sampled': True, 'geometry_revision': revision, 'viscosity': viscosity,
           'reference_pressure': pressure, 'max_scale': max_scale,
           'origin': [ox, oy], 'spacing': h, 'size': [nx, ny], 'ports': []}
fig, axes = plt.subplots(2, 2, figsize=(11, 8), constrained_layout=True)
for ax, (name, center, tangent, rate, radius), meta in zip(axes.flat, geometry, ports):
    tangent = np.array(tangent) / np.linalg.norm(tangent)
    normal = np.array([-tangent[1], tangent[0]])
    lateral = np.linspace(-radius, radius, int(np.ceil(2*radius/h))*4+1)
    actual = sample(np.array(center) + lateral[:, None] * normal)
    expected = 3*rate/(4*radius) * (1-(lateral/radius)**2)
    observed = actual @ tangent
    errors = np.linalg.norm(actual - expected[:, None] * tangent, axis=1)
    normal_velocity = actual @ normal
    tolerance = ((h/radius)**2 + 1e-4) * abs(3*rate/(4*radius)) + 1e-6
    integrated = float(np.sum((observed[:-1]+observed[1:])*.5*np.diff(lateral)))
    metrics['ports'].append({'name': name, 'edge': meta[0], 'center': center,
        'expected_outward_flux': meta[3], 'integrated_outward_flux': integrated*meta[2],
        'max_vector_error': float(errors.max()), 'vector_tolerance': tolerance,
        'max_transverse_velocity': float(np.abs(normal_velocity).max()),
        'wall_speeds': np.linalg.norm(actual[[0,-1]], axis=1).tolist(),
        'interior_center_velocities': {str(d): sample(np.array([center])+[0.,d])[0].tolist() for d in [.5,1.,2.,4.]}})
    ax.plot(lateral, expected, label='analytic reference')
    ax.plot(lateral, observed, label='baked tangent')
    ax.plot(lateral, normal_velocity, label='baked transverse', linestyle=':')
    ax.set(title=name, xlabel='lateral world units', ylabel='world units / s')
    ax.grid(alpha=.2)
    ax.legend(fontsize=8)
fig.savefig(path.with_name('port-profiles.png'), dpi=150)
plt.close(fig)
path.with_name('port-inspection.json').write_text(json.dumps(metrics, indent=2)+'\n')

x = ox + np.arange(nx) * h
y = oy + np.arange(ny) * h
speed = np.linalg.norm(velocity, axis=2)
fig, ax = plt.subplots(figsize=(9, 10), constrained_layout=True)
ax.set_facecolor('#192433')
im = ax.pcolormesh(x, y, np.ma.masked_where(~fluid, speed), shading='nearest', cmap='viridis')
step = 8
xx, yy = np.meshgrid(x[::step], y[::step])
ok = fluid[::step, ::step]
u = velocity[::step, ::step, 0]
v = velocity[::step, ::step, 1]
ax.quiver(xx[ok], yy[ok], u[ok], v[ok], angles='xy', scale_units='xy', scale=.6,
          color='white', width=.0018, alpha=.75)
ax.plot([464, 608, 608, 464, 464], [656, 656, 848, 848, 656], color='#62e7d0', lw=1)
for name, center, _, _, _ in geometry:
    ax.scatter(*center, c='red', s=14)
ax.set(xlim=(464,656), ylim=(848,650), aspect='equal', xlabel='world X', ylabel='world Y (down)',
       title='Boundary-aligned background: prescribed trace and sampled interior')
fig.colorbar(im, ax=ax, label='world speed / s')
fig.savefig(path.with_name('mean-speed.png'), dpi=150)
plt.close(fig)
print(json.dumps(metrics, indent=2))
