use super::{LocalSampleStatus, OpeningSpec, WindowOrigin, WindowSide, WindowSpec, model};
use bytemuck::{Pod, Zeroable};
use std::{cell::Cell, sync::mpsc, time::Instant};
use wgpu::util::DeviceExt;

pub type LbmResult<T> = Result<T, String>;
/// Numerical parameters. `dt` is one lattice step in game-time units.
/// `allow_software` is solely for explicitly requested functional diagnostics.
#[derive(Clone, Copy, Debug)]
pub struct WindowConfig {
    pub dt: f32,
    pub viscosity: f32,
    pub allow_software: bool,
}
/// Row-major cell-center values: dimensionless density and world velocity.
#[derive(Clone, Debug)]
pub struct InitialField {
    pub density: Vec<f32>,
    pub velocity: Vec<[f32; 2]>,
}
/// Row-major solid mask and opening metadata.
#[derive(Clone, Debug)]
pub struct WindowGeometry {
    pub solid: Vec<bool>,
    pub openings: Vec<OpeningSpec>,
}
#[derive(Clone, Copy, Debug)]
pub struct StepInput {
    pub acceleration: [f32; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct LocalSample {
    pub density: f32,
    pub velocity: [f32; 2],
    pub status: LocalSampleStatus,
}
#[derive(Clone, Debug)]
pub struct Diagnostics {
    pub density_min: f32,
    pub density_max: f32,
    pub max_speed: f32,
    pub mass: f32,
    pub opening_fluxes: Vec<f32>,
    pub software_adapter: bool,
    pub adapter_name: String,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    nx: u32,
    ny: u32,
    unused: u32,
    omega: f32,
    force: [f32; 2],
    scale: f32,
    pad: f32,
}

/// Actual operations issued since the last frame reset; durations are CPU wall milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct LbmWorkStats {
    pub lattice_steps: u64,
    pub dispatches: u64,
    pub submissions: u64,
    pub upload_calls: u64,
    pub upload_bytes: u64,
    pub readback_calls: u64,
    pub readback_bytes: u64,
    pub upload_ms: f64,
    pub encoding_ms: f64,
    pub submission_ms: f64,
    /// Includes waiting for previously queued GPU work; not kernel execution time.
    pub readback_wait_ms: f64,
    pub readback_decode_ms: f64,
    pub gpu_kernel_ms: Option<f64>,
}

pub struct LbmWindow {
    work: Cell<LbmWorkStats>,
    spec: WindowSpec,
    config: WindowConfig,
    geometry: WindowGeometry,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    software: bool,
    populations: [wgpu::Buffer; 2],
    current: usize,
    uniform: wgpu::Buffer,
    mask: wgpu::Buffer,
    boundary: wgpu::Buffer,
    boundary_pipeline: wgpu::ComputePipeline,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    groups: [wgpu::BindGroup; 2],
    collide: wgpu::ComputePipeline,
    macros: wgpu::ComputePipeline,
    force: [f32; 2],
    kicks: wgpu::Buffer,
    kick_pipeline: wgpu::ComputePipeline,
    gradient_pipeline: wgpu::ComputePipeline,
    budget: wgpu::Buffer,
    budget_readback: wgpu::Buffer,
    capture: [wgpu::ComputePipeline; 3],
}
impl LbmWindow {
    pub fn new(
        spec: WindowSpec,
        config: WindowConfig,
        initial: InitialField,
        geometry: WindowGeometry,
    ) -> LbmResult<Self> {
        if !config.dt.is_finite()
            || config.dt <= 0.
            || !config.viscosity.is_finite()
            || config.viscosity <= 0.
        {
            return Err("dt and viscosity must be finite and positive".into());
        }
        let n = spec.profile.cells_per_side() as usize;
        validate(n, &initial, &geometry)?;
        if std::env::var("LBMDIAG")
            .ok()
            .is_some_and(|v| matches!(v.as_str(), "7" | "8" | "9" | "10"))
        {
            let mut checked = 0;
            for opening in &geometry.openings {
                for c in opening.cells.clone() {
                    let (x, y, dx, dy) = match opening.side {
                        WindowSide::Left => (0, c as i32, 1, 0),
                        WindowSide::Right => (n as i32 - 1, c as i32, -1, 0),
                        WindowSide::Bottom => (c as i32, 0, 0, 1),
                        WindowSide::Top => (c as i32, n as i32 - 1, 0, -1),
                    };
                    for depth in 1..=2 {
                        let xx = x + depth * dx;
                        let yy = y + depth * dy;
                        if xx < 0
                            || yy < 0
                            || xx >= n as i32
                            || yy >= n as i32
                            || geometry.solid[yy as usize * n + xx as usize]
                        {
                            return Err(format!(
                                "FD stencil unsupported: side={:?} cell={c} inward_depth={depth}",
                                opening.side
                            ));
                        }
                    }
                    checked += 1;
                }
            }
            println!("FD_STENCIL_OK opening_cells={checked} inward_depth=2");
        }

        let mode = std::env::var("LBMDIAG").unwrap_or_default();
        let fit_bytes = if mode == "11" || mode == "12" {
            let expected = super::geometry_blob(n, &geometry, mode == "11");
            let path = std::env::var("LSQ_WEIGHTS").map_err(|_| "LSQ_WEIGHTS is required")?;
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            if data.len() <= expected.len() || data[..expected.len()] != expected {
                return Err("LSQ geometry identity mismatch".into());
            }
            let weights = data[expected.len()..].to_vec();
            if weights.len() % 16 != 0 || weights.len() < n * n * 16 {
                return Err("invalid LSQ weight length".into());
            }
            weights
        } else {
            vec![0u8; 16]
        };
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let software = info.device_type == wgpu::DeviceType::Cpu;
        if software && !config.allow_software {
            return Err("software adapter is only allowed for functional diagnostics".into());
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lbm-window"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let data = initial_populations(spec, config, &initial, [0.; 2]);
        let make = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let pop_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let populations = [
            make("lbm-a", bytemuck::cast_slice(&data), pop_usage),
            make("lbm-b", bytemuck::cast_slice(&data), pop_usage),
        ];
        let uniform = make(
            "lbm-params",
            bytemuck::bytes_of(&params(spec, config, [0.; 2])),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let masks: Vec<u32> = geometry.solid.iter().map(|&x| u32::from(x)).collect();
        let mask = make(
            "lbm-mask",
            bytemuck::cast_slice(&masks),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let boundary_data = boundary_profiles(spec, config, &geometry, None)?;
        let boundary = make(
            "lbm-boundaries",
            bytemuck::cast_slice(&boundary_data),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let kicks = make(
            "lbm-kicks",
            bytemuck::cast_slice(&vec![[0_f32; 2]; n * n]),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lbm-output"),
            size: (n * n * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lbm-readback"),
            size: (n * n * 16) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fit = make("lsq-fit", &fit_bytes, wgpu::BufferUsages::STORAGE);
        let budget_size = (n * n * 30 * 4) as u64;
        let budget = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mass-budget"),
            size: budget_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let budget_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mass-budget-read"),
            size: budget_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let entries: Vec<_> = (0..9)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 0 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding >= 4 && binding != 8,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lbm-layout"),
            entries: &entries,
        });
        let group = |i: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lbm-group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: populations[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: populations[1 - i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: mask.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: boundary.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: kicks.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: fit.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: budget.as_entire_binding(),
                    },
                ],
            })
        };
        let groups = [group(0), group(1)];
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lbm-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lbm-mrt-guo"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lbm.wgsl").into()),
        });
        let pipeline = |name| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pl),
                module: &shader,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let kick_pipeline = pipeline("apply_kicks");
        let collide = pipeline("collide_stream");
        let macros = pipeline("macros");
        let boundary_pipeline = pipeline("reconstruct");
        let gradient_pipeline = pipeline("diagnostic_gradient");
        let capture = [
            pipeline("capture_pre"),
            pipeline("capture_streamed"),
            pipeline("capture_post"),
        ];
        Ok(Self {
            work: Cell::new(LbmWorkStats::default()),
            spec,
            config,
            geometry,
            device,
            queue,
            adapter_name: info.name,
            software,
            populations,
            current: 0,
            uniform,
            mask,
            boundary,
            boundary_pipeline,
            output,
            readback,
            groups,
            collide,
            macros,
            force: [0.; 2],
            kicks,
            kick_pipeline,
            gradient_pipeline,
            budget,
            budget_readback,
            capture,
        })
    }
    pub fn diagnostic_seed(&mut self, values: &[f32]) {
        self.queue.write_buffer(
            &self.populations[self.current],
            0,
            bytemuck::cast_slice(values),
        );
    }
    pub fn reset_work_stats(&self) {
        self.work.set(LbmWorkStats::default());
    }
    pub fn work_stats(&self) -> LbmWorkStats {
        self.work.get()
    }
    fn record(&self, f: impl FnOnce(&mut LbmWorkStats)) {
        let mut stats = self.work.get();
        f(&mut stats);
        self.work.set(stats);
    }
    fn upload(&self, buffer: &wgpu::Buffer, data: &[u8]) {
        let start = Instant::now();
        self.queue.write_buffer(buffer, 0, data);
        self.record(|s| {
            s.upload_calls += 1;
            s.upload_bytes += data.len() as u64;
            s.upload_ms += start.elapsed().as_secs_f64() * 1000.;
        });
    }
    fn submit(&self, encoder: wgpu::CommandEncoder) {
        let start = Instant::now();
        let command = encoder.finish();
        self.record(|s| s.encoding_ms += start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        self.queue.submit([command]);
        self.record(|s| {
            s.submissions += 1;
            s.submission_ms += start.elapsed().as_secs_f64() * 1000.;
        });
    }
    pub fn diagnostic_populations(&self) -> Vec<f32> {
        let size = u64::from(self.spec.profile.cells_per_side().pow(2)) * 9 * 4;
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(&self.populations[self.current], 0, &read, 0, size);
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let data = read.slice(..).get_mapped_range();
        let result = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        read.unmap();
        result
    }
    /// Supply outward normal velocities for each opening cell, in world units.
    /// Each profile must integrate to its corresponding outward flow target.
    pub fn set_opening_profiles(&mut self, flows: &[f32], profiles: &[Vec<f32>]) -> LbmResult<()> {
        if flows.len() != self.geometry.openings.len() {
            return Err("opening target count mismatch".into());
        }
        let mut geometry = self.geometry.clone();
        for (opening, &flow) in geometry.openings.iter_mut().zip(flows) {
            opening.outward_flow = flow;
        }
        let data = boundary_profiles(self.spec, self.config, &geometry, Some(profiles))?;
        self.upload(&self.boundary, bytemuck::cast_slice(&data));
        self.geometry = geometry;
        Ok(())
    }
    pub fn spec(&self) -> WindowSpec {
        self.spec
    }
    pub fn geometry(&self) -> &WindowGeometry {
        &self.geometry
    }
    /// Apply per-cell lattice momentum changes to current populations on the GPU.
    pub fn apply_kicks(&mut self, kicks: &[[f32; 2]]) -> LbmResult<()> {
        let n = self.spec.profile.cells_per_side().pow(2) as usize;
        if kicks.len() != n || kicks.iter().flatten().any(|v| !v.is_finite()) {
            return Err("invalid momentum kicks".into());
        }
        self.upload(&self.kicks, bytemuck::cast_slice(kicks));
        let encoding_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lbm-kicks"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lbm-kicks"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.kick_pipeline);
            pass.set_bind_group(0, &self.groups[self.current], &[]);
            pass.dispatch_workgroups((n as u32).div_ceil(128), 1, 1);
        }
        self.record(|s| {
            s.encoding_ms += encoding_started.elapsed().as_secs_f64() * 1000.;
            s.dispatches += 1;
        });
        self.submit(encoder);
        Ok(())
    }
    /// Advance exactly one lattice step of `config.dt` game-time units.
    pub fn advance(&mut self, input: StepInput) -> LbmResult<()> {
        self.advance_inner(input, false)
    }
    pub fn advance_budget(&mut self, input: StepInput) -> LbmResult<Vec<f32>> {
        self.advance_inner(input, true)?;
        let bytes = u64::from(self.spec.profile.cells_per_side().pow(2)) * 30 * 4;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mass-budget-copy"),
            });
        encoder.copy_buffer_to_buffer(&self.budget, 0, &self.budget_readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        self.budget_readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let data = self.budget_readback.slice(..).get_mapped_range();
        let values = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        self.budget_readback.unmap();
        Ok(values)
    }
    fn capture_into(&self, encoder: &mut wgpu::CommandEncoder, stage: usize, group: usize) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("mass-budget-capture"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.capture[stage]);
        pass.set_bind_group(0, &self.groups[group], &[]);
        pass.dispatch_workgroups(
            self.spec.profile.cells_per_side().pow(2).div_ceil(128),
            1,
            1,
        );
    }
    fn advance_inner(&mut self, input: StepInput, capture: bool) -> LbmResult<()> {
        if !input.acceleration.iter().all(|v| v.is_finite()) {
            return Err("nonfinite acceleration".into());
        }
        self.force = input.acceleration;
        self.upload(
            &self.uniform,
            bytemuck::bytes_of(&params(self.spec, self.config, self.force)),
        );
        let encoding_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lbm-step"),
            });
        if capture {
            self.capture_into(&mut encoder, 0, self.current);
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lbm-step"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.collide);
            pass.set_bind_group(0, &self.groups[self.current], &[]);
            pass.dispatch_workgroups(
                (self.spec.profile.cells_per_side().pow(2)).div_ceil(128),
                1,
                1,
            );
        }
        self.current = 1 - self.current;
        if capture {
            self.capture_into(&mut encoder, 1, self.current);
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lbm-openings"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.boundary_pipeline);
            pass.set_bind_group(0, &self.groups[self.current], &[]);
            pass.dispatch_workgroups(
                (self.spec.profile.cells_per_side().pow(2)).div_ceil(128),
                1,
                1,
            );
        }
        if capture {
            self.capture_into(&mut encoder, 2, self.current);
        }
        self.record(|s| {
            s.encoding_ms += encoding_started.elapsed().as_secs_f64() * 1000.;
            s.dispatches += 2;
            s.lattice_steps += 1;
        });
        self.submit(encoder);
        Ok(())
    }
    /// Snapshot row-major [density, velocity_x, velocity_y, reserved] values.
    pub fn diagnostic_boundary(&self, values: &[[f32; 4]]) {
        self.queue
            .write_buffer(&self.boundary, 0, bytemuck::cast_slice(values));
    }
    pub fn diagnostic_gradient(&self) -> LbmResult<Vec<[f32; 4]>> {
        self.read_field(true)
    }
    pub fn field(&self) -> LbmResult<Vec<[f32; 4]>> {
        self.read_field(false)
    }
    fn read_field(&self, gradient: bool) -> LbmResult<Vec<[f32; 4]>> {
        let n = self.spec.profile.cells_per_side().pow(2);
        let encoding_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lbm-read"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lbm-macros"),
                timestamp_writes: None,
            });
            pass.set_pipeline(if gradient {
                &self.gradient_pipeline
            } else {
                &self.macros
            });
            pass.set_bind_group(0, &self.groups[self.current], &[]);
            pass.dispatch_workgroups(n.div_ceil(128), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, u64::from(n) * 16);
        self.record(|s| {
            s.encoding_ms += encoding_started.elapsed().as_secs_f64() * 1000.;
            s.dispatches += 1;
        });
        self.submit(encoder);
        let wait_started = Instant::now();
        self.record(|s| {
            s.readback_calls += 1;
            s.readback_bytes += u64::from(n) * 16;
        });
        let (tx, rx) = mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        self.record(|s| s.readback_wait_ms += wait_started.elapsed().as_secs_f64() * 1000.);
        let decode_started = Instant::now();
        let data = self.readback.slice(..).get_mapped_range();
        let result = bytemuck::cast_slice::<u8, [f32; 4]>(&data).to_vec();
        drop(data);
        self.readback.unmap();
        self.record(|s| s.readback_decode_ms += decode_started.elapsed().as_secs_f64() * 1000.);
        Ok(result)
    }
    pub fn sample(&self, position: [f32; 2]) -> LbmResult<LocalSample> {
        let o = self.spec.world_bounds()[0];
        let h = self.spec.profile.cell_spacing();
        let n = self.spec.profile.cells_per_side() as usize;
        if !position.iter().all(|x| x.is_finite()) {
            return Err("nonfinite position".into());
        }
        let x = ((position[0] - o[0]) / h).floor() as i64;
        let y = ((position[1] - o[1]) / h).floor() as i64;
        if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
            return Ok(LocalSample {
                density: 0.,
                velocity: [0.; 2],
                status: LocalSampleStatus::OutsideWindow,
            });
        }
        let i = y as usize * n + x as usize;
        if self.geometry.solid[i] {
            return Ok(LocalSample {
                density: 0.,
                velocity: [0.; 2],
                status: LocalSampleStatus::SolidOrInvalid,
            });
        }
        let p = self.field()?[i];
        let valid = p[0] > 0. && p[..3].iter().all(|v| v.is_finite());
        Ok(LocalSample {
            density: p[0],
            velocity: [p[1], p[2]],
            status: if valid {
                LocalSampleStatus::Valid
            } else {
                LocalSampleStatus::SolidOrInvalid
            },
        })
    }
    pub fn snapshot_diagnostics(&self) -> LbmResult<Diagnostics> {
        let f = self.field()?;
        self.diagnostics_from_field(&f)
    }
    /// Calculate diagnostics from an already-read field without a GPU readback.
    pub fn diagnostics_from_field(&self, f: &[[f32; 4]]) -> LbmResult<Diagnostics> {
        if f.len() != self.geometry.solid.len() {
            return Err("invalid field length".into());
        }
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut speed = 0_f32;
        let mut mass = 0_f32;
        for (i, p) in f.iter().enumerate() {
            if self.geometry.solid[i] {
                continue;
            }
            if p[0] <= 0. || !p[..3].iter().all(|v| v.is_finite()) {
                return Err("invalid fluid state".into());
            }
            mass += p[0];
            lo = lo.min(p[0]);
            hi = hi.max(p[0]);
            speed = speed.max(p[1].hypot(p[2]));
        }
        if !lo.is_finite() {
            return Err("window has no fluid cells".into());
        }
        Ok(Diagnostics {
            density_min: lo,
            density_max: hi,
            max_speed: speed,
            mass,
            opening_fluxes: self
                .geometry
                .openings
                .iter()
                .map(|o| {
                    let normal = side_normal(o.side);
                    o.cells
                        .clone()
                        .map(|j| {
                            let i = side_cell(
                                o.side,
                                j as usize,
                                self.spec.profile.cells_per_side() as usize,
                            );
                            (f[i][1] * normal[0] + f[i][2] * normal[1])
                                * self.spec.profile.cell_spacing()
                        })
                        .sum()
                })
                .collect(),
            software_adapter: self.software,
            adapter_name: self.adapter_name.clone(),
        })
    }
    pub fn opening_targets(&self) -> Vec<f32> {
        self.geometry
            .openings
            .iter()
            .map(|o| o.outward_flow)
            .collect()
    }
    /// Copy overlapping populations exactly; initialize newly exposed cells from
    /// `initial` and replace the mask and opening metadata for the new window.
    pub fn shift_to(
        &mut self,
        origin: WindowOrigin,
        initial: InitialField,
        geometry: WindowGeometry,
    ) -> LbmResult<()> {
        let new = WindowSpec {
            origin,
            ..self.spec
        };
        let n = self.spec.profile.cells_per_side() as usize;
        validate(n, &initial, &geometry)?;
        let boundary_data = boundary_profiles(new, self.config, &geometry, None)?;
        let data = initial_populations(new, self.config, &initial, self.force);
        let target = 1 - self.current;
        self.upload(&self.populations[target], bytemuck::cast_slice(&data));
        let [dx, dy] = new.origin_cell_offset_from(self.spec);
        let encoding_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lbm-shift"),
            });
        let x0 = 0_i64.max(-dx);
        let x1 = (n as i64).min(n as i64 - dx);
        let y0 = 0_i64.max(-dy);
        let y1 = (n as i64).min(n as i64 - dy);
        if x1 > x0 && y1 > y0 {
            for q in 0..9 {
                for y in y0..y1 {
                    let src = (q * n * n + (y + dy) as usize * n + (x0 + dx) as usize) * 4;
                    let dst = (q * n * n + y as usize * n + x0 as usize) * 4;
                    encoder.copy_buffer_to_buffer(
                        &self.populations[self.current],
                        src as u64,
                        &self.populations[target],
                        dst as u64,
                        ((x1 - x0) * 4) as u64,
                    );
                }
            }
        }
        self.record(|s| {
            s.encoding_ms += encoding_started.elapsed().as_secs_f64() * 1000.;
        });
        self.submit(encoder);
        let masks: Vec<u32> = geometry.solid.iter().map(|&x| u32::from(x)).collect();
        self.upload(&self.mask, bytemuck::cast_slice(&masks));
        self.upload(&self.boundary, bytemuck::cast_slice(&boundary_data));
        self.current = target;
        self.spec = new;
        self.geometry = geometry;
        Ok(())
    }
}
fn params(spec: WindowSpec, c: WindowConfig, a: [f32; 2]) -> Params {
    let h = spec.profile.cell_spacing();
    let k = c.dt * c.dt / h;
    Params {
        nx: spec.profile.cells_per_side(),
        ny: spec.profile.cells_per_side(),
        unused: 0,
        omega: 1. / (0.5 + 3. * c.viscosity * c.dt / (h * h)),
        force: [a[0] * k, a[1] * k],
        scale: h / c.dt,
        pad: std::env::var("LBMDIAG")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.),
    }
}
fn validate(n: usize, initial: &InitialField, geometry: &WindowGeometry) -> LbmResult<()> {
    if initial.density.len() != n * n
        || initial.velocity.len() != n * n
        || geometry.solid.len() != n * n
    {
        return Err("field or mask length does not match window".into());
    }
    if initial.density.iter().any(|x| !x.is_finite() || *x <= 0.)
        || initial.velocity.iter().flatten().any(|x| !x.is_finite())
    {
        return Err("invalid initial field".into());
    }
    if geometry
        .openings
        .iter()
        .any(|o| o.cells.start >= o.cells.end || o.cells.end > n as u32)
    {
        return Err("invalid opening range".into());
    }
    Ok(())
}
fn initial_populations(
    spec: WindowSpec,
    c: WindowConfig,
    initial: &InitialField,
    acceleration: [f32; 2],
) -> Vec<f32> {
    let n = initial.density.len();
    let k = c.dt / spec.profile.cell_spacing();
    let force = params(spec, c, acceleration).force;
    let mut result = vec![0.; n * 9];
    for i in 0..n {
        let u = initial.velocity[i];
        let eq = model::initial(initial.density[i], [u[0] * k, u[1] * k], force);
        for q in 0..9 {
            result[q * n + i] = eq[q];
        }
    }
    result
}

fn side_normal(side: WindowSide) -> [f32; 2] {
    match side {
        WindowSide::Left => [-1., 0.],
        WindowSide::Right => [1., 0.],
        WindowSide::Bottom => [0., -1.],
        WindowSide::Top => [0., 1.],
    }
}
fn side_cell(side: WindowSide, j: usize, n: usize) -> usize {
    match side {
        WindowSide::Left => j * n,
        WindowSide::Right => j * n + n - 1,
        WindowSide::Bottom => j,
        WindowSide::Top => (n - 1) * n + j,
    }
}
fn boundary_profiles(
    spec: WindowSpec,
    config: WindowConfig,
    geometry: &WindowGeometry,
    profiles: Option<&[Vec<f32>]>,
) -> LbmResult<Vec<[f32; 4]>> {
    let n = spec.profile.cells_per_side() as usize;
    if profiles.is_some_and(|p| p.len() != geometry.openings.len()) {
        return Err("opening profile count mismatch".into());
    }
    let mut data = vec![[0.; 4]; n * n];
    for (k, o) in geometry.openings.iter().enumerate() {
        let normal = side_normal(o.side);
        let dot = o.direction[0] * normal[0] + o.direction[1] * normal[1];
        if !o.outward_flow.is_finite()
            || !o.direction.iter().all(|v| v.is_finite())
            || (o.direction[0].hypot(o.direction[1]) - 1.).abs() > 1e-3
            || dot.abs() < 0.1
            || (o.cells.start == 0
                && std::env::var("LBMDIAG").ok().is_none_or(|v| {
                    v != "3" && v != "4" && v != "6" && v != "7" && v != "9" && v != "11"
                }))
            || (o.cells.end >= n as u32
                && std::env::var("LBMDIAG").ok().is_none_or(|v| {
                    v != "3" && v != "4" && v != "6" && v != "7" && v != "9" && v != "11"
                }))
            || o.cells.start >= o.cells.end
        {
            return Err("invalid or corner opening".into());
        }
        let len = (o.cells.end - o.cells.start) as usize;
        let default: Vec<f32> = (0..len)
            .map(|j| {
                let x = (j as f32 + 0.5) / len as f32;
                x * (1. - x)
            })
            .collect();
        let sum: f32 = default.iter().sum();
        let values = profiles.map(|p| p[k].clone()).unwrap_or_else(|| {
            default
                .iter()
                .map(|w| o.outward_flow * w / (sum * spec.profile.cell_spacing()))
                .collect()
        });
        if values.len() != len
            || values.iter().any(|v| !v.is_finite())
            || (values.iter().sum::<f32>() * spec.profile.cell_spacing() - o.outward_flow).abs()
                > 1e-4 * (1. + o.outward_flow.abs())
        {
            return Err("invalid opening profile or flux".into());
        }
        for (j, &v) in o.cells.clone().zip(&values) {
            let i = side_cell(o.side, j as usize, n);
            if geometry.solid[i] || data[i][0] != 0. {
                return Err("opening overlaps solid or another opening".into());
            }
            let scale = config.dt / spec.profile.cell_spacing();
            let u = [
                v * o.direction[0] / dot * scale,
                v * o.direction[1] / dot * scale,
            ];
            if u[0].hypot(u[1]) >= 0.5 {
                return Err("opening lattice speed too large".into());
            }
            let code = match o.side {
                WindowSide::Left => 1.,
                WindowSide::Right => 2.,
                WindowSide::Bottom => 3.,
                WindowSide::Top => 4.,
            };
            data[i] = [code, u[0], u[1], 0.];
        }
    }
    Ok(data)
}
