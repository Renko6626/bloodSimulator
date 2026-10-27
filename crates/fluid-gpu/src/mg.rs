//! Device-resident block FGMRES. Immutable dynamic-uniform slots allow batches.
use crate::{
    LabResult,
    legacy::Context,
    profile::{Dispatch, Profiler},
};
use blood_fluid::{
    CsrMatrix, PreparedSolver, SolveOptions, SolveReport, SolverCounters, StokesProblem,
    block::BlockPreconditioner,
};
use bytemuck::{Pod, Zeroable};
use std::{
    cell::RefCell,
    num::NonZeroU64,
    sync::{Arc, mpsc},
    time::Instant,
};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct Params {
    n: u32,
    src: u32,
    dst: u32,
    basis: u32,
    count: u32,
    chunks: u32,
    mode: u32,
    add: u32,
    alpha: f32,
    aux: u32,
    ptr: u32,
    col: u32,
    val: u32,
    diag: u32,
    split: u32,
    j: u32,
    restart: u32,
    state: u32,
    tolerance: f32,
    gate: u32,
}
#[derive(Clone, Copy)]
struct MatrixRef {
    rows: u32,
    ptr: u32,
    col: u32,
    val: u32,
}
#[derive(Clone, Copy)]
struct Slots {
    b: u32,
    x: u32,
    tmp: u32,
    r: u32,
}
#[derive(Clone, Copy)]
struct DeviceLevel {
    a: MatrixRef,
    p: Option<MatrixRef>,
    r: Option<MatrixRef>,
    diag: u32,
    omega: f32,
    slots: Slots,
}
#[derive(Clone, Copy)]
struct Step {
    kernel: usize,
    param: u32,
    x: u32,
    y: u32,
    label: &'static str,
}
#[derive(Default)]
struct Arena {
    ptr: Vec<u32>,
    col: Vec<u32>,
    val: Vec<f32>,
}
impl Arena {
    fn matrix(&mut self, a: &CsrMatrix) -> MatrixRef {
        let r = MatrixRef {
            rows: a.rows as u32,
            ptr: self.ptr.len() as u32,
            col: self.col.len() as u32,
            val: self.val.len() as u32,
        };
        self.ptr.extend(&a.ptr);
        self.col.extend(&a.columns);
        self.val.extend(a.values.iter().map(|&v| v as f32));
        r
    }
    fn values(&mut self, x: &[f64]) -> u32 {
        let p = self.val.len() as u32;
        self.val.extend(x.iter().map(|&x| x as f32));
        p
    }
}
struct Plans {
    n: u32,
    nv: u32,
    restart: u32,
    state: u32,
    v: u32,
    z: u32,
    k: MatrixRef,
    coarse: MatrixRef,
    pressure: u32,
    levels: Vec<DeviceLevel>,
    smooth: usize,
    v_cycles: usize,
    params: Vec<Params>,
    stride: u32,
    tolerance: f32,
}
impl Plans {
    fn config(&self) -> Params {
        Params {
            n: self.n,
            chunks: self.n.div_ceil(256),
            restart: self.restart,
            state: self.state,
            tolerance: self.tolerance,
            gate: 1,
            ..Default::default()
        }
    }
    fn push(&mut self, out: &mut Vec<Step>, kernel: usize, p: Params, x: u32, y: u32) {
        let param = self.params.len() as u32 * self.stride;
        self.params.push(p);
        out.push(Step {
            kernel,
            param,
            x,
            y,
            label: match kernel {
                0 if p.ptr == self.k.ptr && p.n == self.n => "stokes_spmv",
                0 if p.ptr == self.k.ptr => "block_velocity_rhs",
                0 if p.ptr == self.coarse.ptr => "mg_coarse",
                0 if self
                    .levels
                    .iter()
                    .any(|l| l.r.is_some_and(|m| m.ptr == p.ptr)) =>
                {
                    "mg_restrict"
                }
                0 if self
                    .levels
                    .iter()
                    .any(|l| l.p.is_some_and(|m| m.ptr == p.ptr)) =>
                {
                    "mg_prolong"
                }
                0 => "mg_residual",
                1 => "dot_chunks",
                2 => "dot_reduce",
                3 if p.mode == 1 => "orthogonalize",
                3 if p.mode == 2 => "solution_update",
                3 if p.mode == 3 => "normalize",
                3 if p.mode == 5 => "clear",
                3 => "vector_copy",
                4 => "mg_smooth",
                5 => "block_pressure",
                6 => "cycle_control",
                7 => "qr",
                8 => "backsolve",
                _ => unreachable!(),
            },
        });
    }
    fn vector(&mut self, out: &mut Vec<Step>, n: u32, src: u32, dst: u32, mode: u32, gate: u32) {
        self.push(
            out,
            3,
            Params {
                n,
                src,
                dst,
                mode,
                alpha: 1.,
                gate,
                ..self.config()
            },
            n.div_ceil(256),
            1,
        );
    }
    fn spmv(
        &mut self,
        out: &mut Vec<Step>,
        a: MatrixRef,
        n: u32,
        src: u32,
        dst: u32,
        aux: u32,
        mode: u32,
        gate: u32,
    ) {
        self.push(
            out,
            0,
            Params {
                n,
                ptr: a.ptr,
                col: a.col,
                val: a.val,
                src,
                dst,
                aux,
                mode,
                gate,
                ..self.config()
            },
            n.div_ceil(256),
            1,
        );
    }
    fn dots(&mut self, out: &mut Vec<Step>, src: u32, count: u32, norm: bool, add: u32, gate: u32) {
        let p = Params {
            src,
            basis: self.v,
            count,
            mode: u32::from(norm),
            add,
            gate,
            ..self.config()
        };
        self.push(out, 1, p, p.chunks, count);
        self.push(out, 2, p, count, 1);
    }
    fn cycle(&mut self, k: usize, out: &mut Vec<Step>) {
        let l = self.levels[k];
        let n = l.a.rows;
        if k + 1 == self.levels.len() {
            self.spmv(out, self.coarse, n, l.slots.b, l.slots.x, 0, 0, 1);
            return;
        }
        let mut current = l.slots.x;
        let mut other = l.slots.tmp;
        for _ in 0..self.smooth {
            self.push(
                out,
                4,
                Params {
                    n,
                    src: current,
                    dst: other,
                    aux: l.slots.b,
                    ptr: l.a.ptr,
                    col: l.a.col,
                    val: l.a.val,
                    diag: l.diag,
                    alpha: l.omega,
                    ..self.config()
                },
                n.div_ceil(256),
                1,
            );
            std::mem::swap(&mut current, &mut other);
        }
        self.spmv(out, l.a, n, current, l.slots.r, l.slots.b, 1, 1);
        let child = self.levels[k + 1];
        self.spmv(
            out,
            l.r.unwrap(),
            child.a.rows,
            l.slots.r,
            child.slots.b,
            0,
            0,
            1,
        );
        self.vector(out, child.a.rows, 0, child.slots.x, 5, 1);
        self.cycle(k + 1, out);
        self.spmv(out, l.p.unwrap(), n, child.slots.x, current, 0, 2, 1);
        for _ in 0..self.smooth {
            self.push(
                out,
                4,
                Params {
                    n,
                    src: current,
                    dst: other,
                    aux: l.slots.b,
                    ptr: l.a.ptr,
                    col: l.a.col,
                    val: l.a.val,
                    diag: l.diag,
                    alpha: l.omega,
                    ..self.config()
                },
                n.div_ceil(256),
                1,
            );
            std::mem::swap(&mut current, &mut other);
        }
        assert_eq!(current, l.slots.x);
    }
    fn start(&mut self) -> Vec<Step> {
        let mut out = Vec::new();
        self.spmv(&mut out, self.k, self.n, 0, 2 * self.n, self.n, 1, 0);
        self.dots(&mut out, 2 * self.n, 1, true, 0, 0);
        self.vector(&mut out, self.n, 2 * self.n, self.v, 3, 0);
        self.push(
            &mut out,
            6,
            Params {
                gate: 0,
                ..self.config()
            },
            1,
            1,
        );
        out
    }
    fn iteration(&mut self, j: u32) -> Vec<Step> {
        let mut out = Vec::new();
        let v = self.v + j * self.n;
        let z = self.z + j * self.n;
        let root = self.levels[0];
        self.push(
            &mut out,
            5,
            Params {
                src: v,
                dst: z,
                split: self.nv,
                diag: self.pressure,
                ..self.config()
            },
            self.n.div_ceil(256),
            1,
        );
        self.spmv(&mut out, self.k, self.nv, z, root.slots.b, v, 1, 1);
        self.vector(&mut out, self.nv, 0, root.slots.x, 5, 1);
        for _ in 0..self.v_cycles {
            self.cycle(0, &mut out);
        }
        self.vector(&mut out, self.nv, root.slots.x, z, 0, 1);
        self.spmv(&mut out, self.k, self.n, z, 2 * self.n, 0, 0, 1);
        for pass in 0..2 {
            self.dots(&mut out, 2 * self.n, j + 1, false, pass, 1);
            let coeff = if pass == 0 { 0 } else { self.restart + 2 };
            self.push(
                &mut out,
                3,
                Params {
                    dst: 2 * self.n,
                    basis: self.v,
                    count: j + 1,
                    mode: 1,
                    diag: coeff,
                    ..self.config()
                },
                self.n.div_ceil(256),
                1,
            );
        }
        self.dots(&mut out, 2 * self.n, 1, true, 0, 1);
        self.vector(
            &mut out,
            self.n,
            2 * self.n,
            self.v + (j + 1) * self.n,
            3,
            1,
        );
        self.push(&mut out, 7, Params { j, ..self.config() }, 1, 1);
        out
    }
    fn finish(&mut self) -> Vec<Step> {
        let mut out = Vec::new();
        self.push(
            &mut out,
            8,
            Params {
                gate: 0,
                ..self.config()
            },
            1,
            1,
        );
        self.push(
            &mut out,
            3,
            Params {
                dst: 0,
                basis: self.z,
                count: self.restart,
                mode: 2,
                diag: self.state - self.restart,
                gate: 0,
                ..self.config()
            },
            self.n.div_ceil(256),
            1,
        );
        out
    }
}

pub struct MgSolver {
    context: Arc<Context>,
    problem: Arc<StokesProblem>,
    scale: Vec<f64>,
    options: SolveOptions,
    work: wgpu::Buffer,
    scalar: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipelines: Vec<wgpu::ComputePipeline>,
    start: Vec<Step>,
    iterations: Vec<Vec<Step>>,
    finish: Vec<Step>,
    state: u32,
    scalar_len: usize,
    levels: Vec<usize>,
    counters: SolverCounters,
    profile: Option<RefCell<Profiler>>,
}
impl MgSolver {
    pub fn new(
        context: Arc<Context>,
        problem: Arc<StokesProblem>,
        options: SolveOptions,
        profiling: bool,
    ) -> LabResult<Self> {
        if options.restart == 0
            || options.restart > 64
            || options.max_iterations == 0
            || options.tolerance <= 0.
            || !options.tolerance.is_finite()
        {
            return Err("invalid FGMRES options".into());
        }
        let pre = BlockPreconditioner::with_cycles(&problem, options.v_cycles)?;
        let n = problem.matrix.rows as u32;
        let m = options.restart as u32;
        let state = 2 * (m + 2) + (m + 1) * m + 2 * m + (m + 1) + m;
        let scalar_len = (state + 8) as usize;
        let mut arena = Arena::default();
        let k = arena.matrix(&pre.matrix);
        let mut levels = Vec::new();
        let mut offset = (3 + 2 * (m + 1)) * n;
        for l in &pre.hierarchy.levels {
            let a = arena.matrix(&l.matrix);
            let diag = arena.values(&l.inverse_diagonal);
            let p = l.prolongation.as_ref().map(|p| arena.matrix(p));
            let r = l.restriction.as_ref().map(|r| arena.matrix(r));
            let slots = Slots {
                b: offset,
                x: offset + a.rows,
                tmp: offset + 2 * a.rows,
                r: offset + 3 * a.rows,
            };
            offset += 4 * a.rows;
            levels.push(DeviceLevel {
                a,
                p,
                r,
                diag,
                omega: l.omega as f32,
                slots,
            });
        }
        let coarse = arena.matrix(&pre.hierarchy.coarse_inverse);
        let pressure = arena.values(&pre.pressure_inverse);
        if arena.val.iter().any(|v| !v.is_finite()) {
            return Err("non-finite f32 MG coefficients".into());
        }
        let device = &context.device;
        let stride = device.limits().min_uniform_buffer_offset_alignment;
        if offset as u64 * 4 > device.limits().max_storage_buffer_binding_size as u64 {
            return Err("FGMRES workspace exceeds device storage limit".into());
        }
        let mut plans = Plans {
            n,
            nv: problem.velocity_count as u32,
            restart: m,
            state,
            v: 3 * n,
            z: (3 + m + 1) * n,
            k,
            coarse,
            pressure,
            levels,
            smooth: pre.hierarchy.smooth_steps,
            v_cycles: options.v_cycles,
            params: Vec::new(),
            stride,
            tolerance: options.tolerance as f32,
        };
        let start = plans.start();
        let iterations: Vec<Vec<Step>> = (0..m).map(|j| plans.iteration(j)).collect();
        let finish = plans.finish();
        let mut uniforms = vec![0_u8; plans.params.len() * stride as usize];
        for (i, p) in plans.params.iter().enumerate() {
            let at = i * stride as usize;
            uniforms[at..at + std::mem::size_of::<Params>()].copy_from_slice(bytemuck::bytes_of(p));
        }
        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let row = init(
            "MG row pointers",
            bytemuck::cast_slice(&arena.ptr),
            wgpu::BufferUsages::STORAGE,
        );
        let col = init(
            "MG columns",
            bytemuck::cast_slice(&arena.col),
            wgpu::BufferUsages::STORAGE,
        );
        let val = init(
            "MG values",
            bytemuck::cast_slice(&arena.val),
            wgpu::BufferUsages::STORAGE,
        );
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let work = buffer(
            "FGMRES vectors",
            offset as u64 * 4,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        );
        let scalar = buffer(
            "FGMRES scalars",
            scalar_len as u64 * 4,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        );
        let partial = buffer(
            "FGMRES reduction",
            (m + 1) as u64 * n.div_ceil(256) as u64 * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let params = init(
            "Immutable pass parameters",
            &uniforms,
            wgpu::BufferUsages::UNIFORM,
        );
        let entries: Vec<_> = (0..7)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 6 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding < 3,
                        }
                    },
                    has_dynamic_offset: binding == 6,
                    min_binding_size: if binding == 6 {
                        NonZeroU64::new(std::mem::size_of::<Params>() as u64)
                    } else {
                        None
                    },
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MG layout"),
            entries: &entries,
        });
        let buffers = [&row, &col, &val, &work, &partial, &scalar];
        let mut entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &params,
                offset: 0,
                size: NonZeroU64::new(std::mem::size_of::<Params>() as u64),
            }),
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FGMRES MG kernels"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mg.wgsl").into()),
        });
        let pipelines = [
            "spmv",
            "project",
            "reduce",
            "vector",
            "jacobi",
            "pressure",
            "cycle_init",
            "qr",
            "backsolve",
        ]
        .iter()
        .map(|name| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        })
        .collect();
        let level_sizes = pre.hierarchy.levels.iter().map(|l| l.matrix.rows).collect();
        let max_batch =
            (iterations.iter().map(Vec::len).max().unwrap() * 8).max(start.len() + finish.len());
        let profile = profiling.then(|| RefCell::new(Profiler::new(&context, max_batch)));
        Ok(Self {
            context,
            problem,
            scale: pre.scale,
            options,
            work,
            scalar,
            group,
            pipelines,
            start,
            iterations,
            finish,
            state,
            scalar_len,
            levels: level_sizes,
            counters: Default::default(),
            profile,
        })
    }
    fn encode<'a>(&self, steps: impl Iterator<Item = &'a Step>) -> wgpu::CommandBuffer {
        let timer = Instant::now();
        let mut profile = self.profile.as_ref().map(RefCell::borrow_mut);
        let from = profile.as_ref().map_or(0, |p| p.pending.len());
        let mut labels = Vec::new();
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        for s in steps {
            let index = (from + labels.len()) as u32 * 2;
            let timestamps = profile.as_ref().and_then(|p| p.queries.as_ref()).map(|q| {
                assert!((index as usize / 2) < q.capacity);
                wgpu::ComputePassTimestampWrites {
                    query_set: &q.set,
                    beginning_of_pass_write_index: Some(index),
                    end_of_pass_write_index: Some(index + 1),
                }
            });
            if timestamps.is_some() {
                labels.push(Dispatch {
                    label: s.label,
                    groups: s.x as u64 * s.y as u64,
                });
            }
            // Pass boundaries establish storage dependencies between the kernels.
            let detail_timer = profile.as_ref().map(|_| Instant::now());
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: timestamps,
            });
            let begin_time = detail_timer.map(|t| t.elapsed());
            let detail_timer = profile.as_ref().map(|_| Instant::now());
            pass.set_pipeline(&self.pipelines[s.kernel]);
            pass.set_bind_group(0, &self.group, &[s.param]);
            let state_time = detail_timer.map(|t| t.elapsed());
            let detail_timer = profile.as_ref().map(|_| Instant::now());
            pass.dispatch_workgroups(s.x, s.y, 1);
            let dispatch_time = detail_timer.map(|t| t.elapsed());
            let detail_timer = profile.as_ref().map(|_| Instant::now());
            drop(pass);
            let end_time = detail_timer.map(|t| t.elapsed());
            if let Some(p) = profile.as_mut() {
                p.detail("encode_begin_pass_ms", begin_time.unwrap());
                p.detail("encode_set_pipeline_bind_group_ms", state_time.unwrap());
                p.detail("encode_dispatch_ms", dispatch_time.unwrap());
                p.detail("encode_end_pass_ms", end_time.unwrap());
            }
        }
        if let Some(p) = profile.as_mut() {
            if let Some(q) = &p.queries {
                let count = labels.len() as u32 * 2;
                // Resolve offset must be 256-aligned. Copy each result range into
                // its own readback slot so finish + next start cannot overwrite it.
                encoder.resolve_query_set(
                    &q.set,
                    from as u32 * 2..from as u32 * 2 + count,
                    &q.resolve,
                    0,
                );
                encoder.copy_buffer_to_buffer(
                    &q.resolve,
                    0,
                    &q.readback,
                    from as u64 * 16,
                    labels.len() as u64 * 16,
                );
            }
            p.pending.extend(labels);
        }
        let finish_timer = Instant::now();
        let buffer = encoder.finish();
        if let Some(p) = profile.as_mut() {
            p.detail("encode_finish_ms", finish_timer.elapsed());
            p.mark("command_encode_ms", timer.elapsed());
        }
        buffer
    }
    fn mark(&self, label: &'static str, timer: Instant) {
        if let Some(p) = &self.profile {
            p.borrow_mut().mark(label, timer.elapsed());
        }
    }
    fn detail(&self, label: &'static str, timer: Instant) {
        if let Some(p) = &self.profile {
            p.borrow_mut().detail(label, timer.elapsed());
        }
    }
    fn submit(&mut self, buffer: wgpu::CommandBuffer, dispatches: usize) {
        let timer = Instant::now();
        self.context.queue.submit([buffer]);
        self.mark("queue_submit_ms", timer);
        self.counters.submissions += 1;
        self.counters.dispatches += dispatches as u64;
    }
    fn read(&mut self, scalar: bool, offset: u64, size: u64) -> LabResult<Vec<f32>> {
        let start = Instant::now();
        let staging = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MG readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.detail("readback_create_buffer_ms", start);
        let timer = Instant::now();
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(
            if scalar { &self.scalar } else { &self.work },
            offset,
            &staging,
            0,
            size,
        );
        self.detail("readback_encode_copy_ms", timer);
        let timer = Instant::now();
        let command = encoder.finish();
        self.detail("readback_finish_ms", timer);
        self.mark("readback_prepare_ms", start);
        let timer = Instant::now();
        self.context.queue.submit([command]);
        self.mark("queue_submit_ms", timer);
        let timer = Instant::now();
        let (tx, rx) = mpsc::sync_channel(1);
        staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.context
            .device
            .poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        self.mark("readback_map_poll_ms", timer);
        let timer = Instant::now();
        let mapped = staging.slice(..).get_mapped_range();
        let result = bytemuck::cast_slice::<u8, f32>(&mapped).to_vec();
        drop(mapped);
        staging.unmap();
        self.mark("readback_decode_ms", timer);
        self.counters.submissions += 1;
        self.counters.readbacks += 1;
        self.counters.readback_bytes += size;
        self.counters.readback_wait_ms += start.elapsed().as_secs_f64() * 1000.;
        if let Some(p) = &self.profile {
            p.borrow_mut().collect(&self.context)?;
        }
        Ok(result)
    }
    fn solve_inner(&mut self, b: &[f64], out: &mut [f64]) -> LabResult<SolveReport> {
        let solve_timer = Instant::now();
        if let Some(p) = &self.profile {
            p.borrow_mut().reset();
        }
        let n = self.problem.matrix.rows;
        if b.len() != n || out.len() != n {
            return Err("invalid vector length".into());
        }
        let rhs: Vec<f32> = b
            .iter()
            .zip(&self.scale)
            .map(|(b, d)| (b * d) as f32)
            .collect();
        if rhs.iter().any(|x| !x.is_finite()) {
            return Err("non-finite f32 RHS".into());
        }
        let bnorm = rhs.iter().map(|&x| (x as f64).powi(2)).sum::<f64>().sqrt();
        self.context
            .queue
            .write_buffer(&self.work, 0, bytemuck::cast_slice(&vec![0_f32; n]));
        self.context
            .queue
            .write_buffer(&self.work, (n * 4) as u64, bytemuck::cast_slice(&rhs));
        let mut scalar = vec![0_f32; self.scalar_len];
        scalar[self.state as usize] = bnorm as f32;
        self.context
            .queue
            .write_buffer(&self.scalar, 0, bytemuck::cast_slice(&scalar));
        self.mark("rhs_prepare_upload_ms", solve_timer);
        self.counters = Default::default();
        let mut last;
        let mut status = "iteration_limit";
        loop {
            let cmd = self.encode(self.start.iter());
            self.submit(cmd, self.start.len());
            last = self.read(true, self.state as u64 * 4, 32)?;
            if last.iter().any(|x| !x.is_finite()) {
                return Err("non-finite device FGMRES state".into());
            }
            if last[1] as f64 <= self.options.tolerance * bnorm.max(1e-30) {
                status = "converged";
                break;
            }
            let done = last[3] as usize;
            if done >= self.options.max_iterations {
                break;
            }
            let count = self.options.restart.min(self.options.max_iterations - done);
            for from in (0..count).step_by(8) {
                let to = (from + 8).min(count);
                let steps: usize = self.iterations[from..to].iter().map(Vec::len).sum();
                let cmd = self.encode(self.iterations[from..to].iter().flatten());
                self.submit(cmd, steps);
                last = self.read(true, self.state as u64 * 4, 32)?;
                if last.iter().any(|x| !x.is_finite()) || last[6] != 0. {
                    return Err("device FGMRES Arnoldi breakdown".into());
                }
                if last[4] < 0.5 {
                    break;
                }
            }
            let cmd = self.encode(self.finish.iter());
            self.submit(cmd, self.finish.len());
        }
        let y = self.read(false, 0, n as u64 * 4)?;
        let timer = Instant::now();
        for i in 0..n {
            out[i] = y[i] as f64 * self.scale[i];
        }
        self.mark("solution_map_ms", timer);
        let timer = Instant::now();
        if status == "converged"
            && !self
                .problem
                .audit(b, out, None, self.options.tolerance)
                .linear_residual_accepted
        {
            status = "original_residual_failed";
        }
        self.mark("original_audit_ms", timer);
        Ok(SolveReport {
            backend: "gpu-fgmres-mg".into(),
            status: status.into(),
            iterations: last[3] as usize,
            stopping_relative: Some(last[1] as f64 / bnorm.max(1e-30)),
            multigrid_levels: self.levels.clone(),
            counters: self.counters.clone(),
            profile: self
                .profile
                .as_ref()
                .map(|p| p.borrow().report(solve_timer.elapsed())),
        })
    }
}
impl PreparedSolver for MgSolver {
    fn solve(&mut self, b: &[f64], out: &mut [f64]) -> Result<SolveReport, String> {
        self.solve_inner(b, out).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn mg_shader_validates_without_device() {
        let m = wgpu::naga::front::wgsl::parse_str(include_str!("mg.wgsl")).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&m)
        .unwrap();
    }
}
