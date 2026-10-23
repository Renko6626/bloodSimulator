#!/usr/bin/env python3
"""Summarize the archived LBM/Rapier CSVs and plot only measured snapshots."""
import csv
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / 'results'
DT = 1 / 2048
COLORS = {'relax': '#285c92', 'reversal': '#d17b32', 'squeeze': '#547d58'}


def read(path):
    with path.open(newline='') as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise ValueError(f'empty CSV: {path}')
    return {key: np.array([float(row[key]) for row in rows]) for key in rows[0] if key not in {'scenario'}}


def checked(table, path):
    for key, values in table.items():
        if not np.all(np.isfinite(values)):
            raise ValueError(f'nonfinite {key} in {path}')


def nearest_p95(values):
    return float(np.sort(values)[math.ceil(0.95 * len(values)) - 1])


def timing(table):
    return {key: {'mean_ms': float(np.mean(table[key][30:])), 'p95_ms': nearest_p95(table[key][30:]),
                  'max_ms': float(np.max(table[key][30:]))}
            for key in ('fluid_ms', 'rapier_ms', 'whole_ms')}


def summarize_moving(name):
    path = RESULTS / 'moving' / f'{name}.csv'
    t = read(path)
    checked(t, path)
    field_path = path.with_name(path.name + '.field.csv')
    field = read(field_path)
    checked(field, field_path)
    if len(field['x']) != 1024 or np.min(field['rho']) <= 0:
        raise ValueError(f'{name}: invalid final field')
    eligible = t['time'] >= 0.1
    crossing = t['grid_cross'] == 1
    force = t['fluid_impulse_x'] / DT
    force_jump = np.abs(np.diff(force, prepend=force[0]))
    input_jump = np.abs(np.diff(t['target_speed'], prepend=t['target_speed'][0]))
    # This bound removes the part that can be directly caused by the commanded speed change.
    gamma = 4.0 if name == 'smooth-g4' else 0.25
    residual_jump = np.maximum(0, force_jump - gamma * input_jump)
    def jump_stats(mask):
        return {'count': int(mask.sum()), 'max_delta_force': float(np.max(force_jump[mask])),
                'max_after_input_bound': float(np.max(residual_jump[mask]))} if np.any(mask) else None
    event = {}
    for label, col in [('center', 'center_seam_cross'), ('marker', 'marker_seam_cross')]:
        ids = np.flatnonzero(t[col] == 1)
        event[label] = [{'step': int(t['step'][i]), 'time': float(t['time'][i]),
                         'delta_force': float(force_jump[i]),
                         'position_unwrapped': float(t['center_x'][i])} for i in ids]
    return {
        'steps': len(t['step']), 'duration_s': float(len(t['step']) * DT),
        'finite_final_field_cells': len(field['x']),
        'rho_min': float(np.min(t['minrho'])), 'rho_max': float(np.max(t['maxrho'])),
        'max_fluid_speed': float(np.max(t['max_fluid_speed'])),
        'max_abs_mass_drift': float(np.max(np.abs(t['mass_drift']))),
        'max_exchange_impulse_error': float(np.max(np.hypot(t['exchange_error_x'], t['exchange_error_y']))),
        'max_kernel_weight_error': float(np.max(t['kernel_weight_error'])),
        'grid_cross_after_0p1': jump_stats(eligible & crossing),
        'other_after_0p1': jump_stats(eligible & ~crossing),
        'seam_events': event,
    }


def summarize_rapier(name, directory=None):
    directory = RESULTS / 'rapier' if directory is None else directory
    path = directory / f'{name}.csv'
    t = read(path)
    checked(t, path)
    field_path = directory / f'{name}_field.csv'
    field = read(field_path)
    checked(field, field_path)
    if len(field['frame']) != 12 * 1024 or np.min(field['rho']) <= 0:
        raise ValueError(f'{name}: invalid field snapshots')
    if len(t['frame']) != 360 or not np.array_equal(t['frame'], np.arange(1, 361)):
        raise ValueError(f'{name}: expected 360 consecutive frames')
    fluid_clock = np.cumsum(t['fluid_steps']) * DT
    frame_clock = t['frame'] / 60
    mismatch = fluid_clock - frame_clock
    if not np.allclose(mismatch, t['fluid_time_mismatch'], atol=5e-10, rtol=0):
        raise ValueError(f'{name}: reported fluid time mismatch differs from cumulative steps')
    if np.max(np.abs(mismatch)) >= DT:
        raise ValueError(f'{name}: fluid clock drift exceeds one lattice step')
    initial_area = float(t['area'][0] / (1 + t['area_rel_error'][0]))
    # Initial outline provides the actual t=0 signed area; frame-1 estimate above is just backup.
    outline_path = directory / f'{name}_outline.csv'
    outline = read(outline_path)
    checked(outline, outline_path)
    x0, y0 = outline['x'][:12], outline['y'][:12]
    signed0 = 0.5 * np.sum(x0 * np.roll(y0, -1) - y0 * np.roll(x0, -1))
    initial_area = abs(float(signed0))
    if not initial_area > 0:
        raise ValueError(f'{name}: zero initial area')
    area_signed = {}
    for frame in np.unique(outline['frame']):
        mask = outline['frame'] == frame
        x, y = outline['x'][mask], outline['y'][mask]
        if len(x) != 12:
            raise ValueError(f'{name}: incomplete outline at frame {frame}')
        area_signed[int(frame)] = float(0.5 * np.sum(x * np.roll(y, -1) - y * np.roll(x, -1)))
    if any(a * signed0 <= 0 for a in area_signed.values()):
        raise ValueError(f'{name}: outline flipped')
    out = {
        'frames': len(t['frame']), 'duration_s': float(t['t'][-1]),
        'finite_field_snapshots': len(np.unique(field['frame'])),
        'fluid_steps': int(np.sum(t['fluid_steps'])),
        'fluid_time_s': float(fluid_clock[-1]),
        'max_abs_clock_error_s': float(np.max(np.abs(mismatch))),
        'max_exchange_impulse_mismatch': float(np.max(t['exchange_impulse_mismatch'])),
        'rho_min': float(np.min(t['min_rho'])), 'rho_max': float(np.max(t['max_rho'])),
        'max_fluid_speed': float(np.max(t['max_fluid_speed'])),
        'initial_area': initial_area,
        'max_abs_area_relative_error': float(np.max(t['area_rel_error'])),
        'final_area_relative_error': float(t['area'][-1] / initial_area - 1),
        'max_node_speed': float(np.max(t['max_node_speed'])),
        'max_node_jump_per_frame': float(np.max(t['max_node_jump'])),
        'timing_excluding_first_30_frames': timing(t),
    }
    if name == 'squeeze':
        idle = t['t'] <= 0.5 + 1e-10
        out['pre_contact_static_0_to_0p5s'] = {
            'frames': int(idle.sum()),
            'max_center_displacement': float(np.max(np.hypot(t['center_x'][idle] - 0.5, t['center_y'][idle] - 0.5))),
            'max_center_speed': float(np.max(np.hypot(t['velocity_x'][idle], t['velocity_y'][idle]))),
            'max_node_jump_per_frame': float(np.max(t['max_node_jump'][idle])),
            'max_abs_area_relative_error': float(np.max(t['area_rel_error'][idle])),
            'max_fluid_speed': float(np.max(t['max_fluid_speed'][idle])),
        }
        after = t['t'] >= 2.5
        out['post_release_max_abs_area_relative_error'] = float(np.max(t['area_rel_error'][after]))
        out['max_node_jump_frame'] = int(t['frame'][np.argmax(t['max_node_jump'])])
    if name == 'reversal':
        ids = np.argsort(np.hypot(t['accel_x'], t['accel_y']))[-5:][::-1]
        out['top_center_acceleration_frames'] = [{'frame': int(t['frame'][i]), 't': float(t['t'][i]),
                                                    'accel': float(np.hypot(t['accel_x'][i], t['accel_y'][i])),
                                                    'drive_force_x': float(t['drive_force_x'][i])} for i in ids]
    return out, t, outline


def compare_precision(name, reference, candidate, reference_outline, candidate_outline):
    if not np.array_equal(reference['frame'], candidate['frame']):
        raise ValueError(f'{name}: f32/f64 frames differ')
    if not np.array_equal(reference_outline['frame'], candidate_outline['frame']) or not np.array_equal(reference_outline['node'], candidate_outline['node']):
        raise ValueError(f'{name}: f32/f64 outline frames or nodes differ')
    frame_position_difference = np.hypot(candidate['center_x'] - reference['center_x'],
                                         candidate['center_y'] - reference['center_y'])
    node_position_difference = np.hypot(candidate_outline['x'] - reference_outline['x'],
                                        candidate_outline['y'] - reference_outline['y'])
    ref_field = read(RESULTS / 'rapier' / f'{name}_field.csv')
    f32_field = read(RESULTS / 'f32' / f'{name}_field.csv')
    checked(ref_field, f'f64 {name} field')
    checked(f32_field, f'f32 {name} field')
    for key in ('frame', 'x', 'y'):
        if not np.array_equal(ref_field[key], f32_field[key]):
            raise ValueError(f'{name}: f32/f64 field {key} alignment differs')
    du = f32_field['velocity_x'] - ref_field['velocity_x']
    dv = f32_field['velocity_y'] - ref_field['velocity_y']
    ref_norm2 = np.sum(ref_field['velocity_x'] ** 2 + ref_field['velocity_y'] ** 2)
    difference_norm2 = np.sum(du ** 2 + dv ** 2)
    relative_l2 = float(np.sqrt(difference_norm2 / ref_norm2)) if ref_norm2 > 0 else None
    field_frames = np.unique(ref_field['frame'])
    ref_mass = np.array([np.mean(ref_field['rho'][ref_field['frame'] == frame]) for frame in field_frames])
    f32_mass = np.array([np.mean(f32_field['rho'][f32_field['frame'] == frame]) for frame in field_frames])
    def bounds(values):
        return {'min': float(np.min(values)), 'max': float(np.max(values))}
    return {
        'max_center_position_difference': float(np.max(frame_position_difference)),
        'max_center_x_difference': float(np.max(np.abs(candidate['center_x'] - reference['center_x']))),
        'max_center_velocity_vector_difference': float(np.max(np.hypot(candidate['velocity_x'] - reference['velocity_x'],
                                                                       candidate['velocity_y'] - reference['velocity_y']))),
        'max_node_position_difference_from_saved_outlines': float(np.max(node_position_difference)),
        'outline_snapshots': len(np.unique(reference_outline['frame'])),
        'max_absolute_area_difference': float(np.max(np.abs(candidate['area'] - reference['area']))),
        'max_area_relative_error_curve_difference': float(np.max(np.abs(candidate['area_rel_error'] - reference['area_rel_error']))),
        'field_velocity_aggregate_relative_l2': relative_l2,
        'field_velocity_reference_l2_norm': float(np.sqrt(ref_norm2)),
        'field_velocity_max_absolute_vector_difference': float(np.max(np.hypot(du, dv))),
        'field_snapshots': len(field_frames),
        'field_mass_error_f64_bounds': bounds(ref_mass - 1),
        'field_mass_error_f32_bounds': bounds(f32_mass - 1),
        'field_mass_f32_minus_f64_bounds': bounds(f32_mass - ref_mass),
    }


def compare_frozen():
    new = read(RESULTS / 'moving/frozen-g0.25.csv.field.csv')
    old = read(ROOT.parent / '2026-10-23-lbm/results/optimized/fields/blob4_n32_tau2_g0.25.csv')
    checked(new, 'new frozen field')
    checked(old, 'old optimized field')
    if len(new['x']) != 1024 or len(old['x']) != 1024 or not np.array_equal(new['x'], old['x']) or not np.array_equal(new['y'], old['y']):
        raise ValueError('frozen field grids differ')
    return {'cells': 1024, 'steps': 3190, 'max_abs_exported_difference': {
        'u': float(np.max(np.abs(new['u'] - old['u']))),
        'v': float(np.max(np.abs(new['v'] - old['v']))),
        'rho': float(np.max(np.abs(new['rho'] - old['density']))),
    }, 'csv_precision': '12 digits after decimal in scientific notation'}


def make_figure(tables, outlines):
    plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 10, 'axes.spines.top': False,
                         'axes.spines.right': False, 'svg.fonttype': 'none'})
    fig = plt.figure(figsize=(11.7, 8.8), layout='constrained')
    gs = fig.add_gridspec(3, 3, height_ratios=[1.2, 1.05, 1.2])
    ax_v = fig.add_subplot(gs[0, :]); ax_a = fig.add_subplot(gs[1, :])
    for name in ('relax', 'reversal', 'squeeze'):
        t = tables[name]
        ax_v.plot(t['t'], t['velocity_x'], color=COLORS[name], lw=1.8, label=name.capitalize())
        ax_a.plot(t['t'], 100 * t['area_rel_error'], color=COLORS[name], lw=1.8, label=name.capitalize())
    ax_v.axhline(0, color='#777777', lw=.8)
    for when in (1.5, 3.0):
        ax_v.axvline(when, color='#aaadaf', lw=.8, ls='--')
    ax_v.set(xlim=(0, 6), ylabel='Center x velocity (length/s)', title='Single-cell dynamics, 32² LBM + Rapier')
    ax_v.legend(ncol=3, frameon=False, loc='upper right')
    ax_a.set(xlim=(0, 6), xlabel='Model time (s)', ylabel='Absolute area deviation (%)')
    ax_a.axvspan(.5, 2.5, color='#e8ece8', alpha=.5, zorder=-1)
    ax_a.text(.55, max(100 * tables['squeeze']['area_rel_error']) * .92, 'plate motion', color='#547d58', fontsize=9)
    for ax in (ax_v, ax_a):
        ax.grid(axis='y', color='#e7e9eb', lw=.7)
    outline = outlines['squeeze']
    available = sorted(set(int(v) for v in outline['frame']))
    desired = [0, 75, 150]
    for j, target in enumerate(desired):
        frame = min(available, key=lambda x: abs(x - target))
        ax = fig.add_subplot(gs[2, j])
        mask = outline['frame'] == frame
        x, y = outline['x'][mask], outline['y'][mask]
        ax.fill(x, y, facecolor='#dce9dd', edgecolor=COLORS['squeeze'], lw=1.8)
        ax.scatter(x, y, s=9, color=COLORS['squeeze'], zorder=3)
        top = outline['plate_top_y'][mask][0]; bottom = outline['plate_bottom_y'][mask][0]
        if np.isfinite(top) and np.isfinite(bottom):
            ax.hlines([top, bottom], .10, .90, color='#626b73', lw=3)
        ax.set(xlim=(.27, .73), ylim=(.27, .73), aspect='equal', title=f'Squeeze t={frame/60:.2f} s', xlabel='x')
        if j == 0: ax.set_ylabel('y')
        else: ax.set_yticklabels([])
        ax.grid(color='#edf0ed', lw=.5)
    fig.savefig(RESULTS / 'overview.png', dpi=175)
    fig.savefig(RESULTS / 'overview.svg')
    plt.close(fig)


def make_animation(outline):
    frames = sorted(set(int(v) for v in outline['frame']))
    images = []
    scale = 500
    for frame in frames:
        mask = outline['frame'] == frame
        xy = list(zip(outline['x'][mask], outline['y'][mask]))
        image = Image.new('RGB', (480, 500), '#f8faf8')
        draw = ImageDraw.Draw(image)
        def pix(x, y): return (int(30 + (x - .2) * scale), int(450 - (y - .2) * scale))
        draw.rectangle((30, 50, 430, 450), outline='#c7cfca', width=2)
        top = float(outline['plate_top_y'][mask][0]); bottom = float(outline['plate_bottom_y'][mask][0])
        draw.line((pix(.25, top), pix(.75, top)), fill='#626b73', width=8)
        draw.line((pix(.25, bottom), pix(.75, bottom)), fill='#626b73', width=8)
        draw.polygon([pix(x, y) for x, y in xy], fill='#dce9dd', outline='#547d58', width=3)
        for x, y in xy:
            px, py = pix(x, y); draw.ellipse((px-3, py-3, px+3, py+3), fill='#547d58')
        draw.text((30, 18), f'Squeeze: t={frame/60:.2f} s', fill='#22312b')
        images.append(image.convert('P', palette=Image.Palette.ADAPTIVE, colors=32))
    images[0].save(RESULTS / 'squeeze.gif', save_all=True, append_images=images[1:], duration=100, loop=0, optimize=True)


def main():
    summary = {'source': 'archived CSVs under results/', 'moving': {}, 'rapier': {},
               'timing_note': 'First 30 frames excluded from timing only; all frames included in stability checks. Whole timing includes frame diagnostics, not CSV writes or snapshots.'}
    for name in ('smooth-g0.25', 'seam-g0.25', 'smooth-g4', 'frozen-g0.25'):
        summary['moving'][name] = summarize_moving(name)
    tables = {}; outlines = {}
    for name in ('relax', 'reversal', 'squeeze'):
        summary['rapier'][name], tables[name], outlines[name] = summarize_rapier(name)
    f32_directory = RESULTS / 'f32'
    if f32_directory.is_dir():
        summary['precision_f32'] = {
            'method': 'Only the fluid module import changes; frame, saved outline, and field rows are aligned with the f64 run. Node differences cover saved outlines every 0.1 s. Field mass is mean rho over 1024 cells at each saved snapshot. Aggregate velocity relative L2 uses all saved field cells and times together, so zero-input snapshots add zero without a zero denominator.',
            'timing_note': 'Each precision variant was run sequentially on a nonexclusive CPU; timing differences are observational, not a speedup estimate.',
            'scenes': {},
        }
        for name in ('relax', 'reversal', 'squeeze'):
            f32_summary, f32_table, f32_outline = summarize_rapier(name, f32_directory)
            summary['precision_f32']['scenes'][name] = {
                'f32_stability_and_timing': f32_summary,
                'difference_from_f64': compare_precision(name, tables[name], f32_table, outlines[name], f32_outline),
            }
    summary['frozen_comparison'] = compare_frozen()
    (RESULTS / 'analysis.json').write_text(json.dumps(summary, indent=2) + '\n')
    make_figure(tables, outlines)
    make_animation(outlines['squeeze'])
    print(json.dumps(summary, indent=2))


if __name__ == '__main__': main()
