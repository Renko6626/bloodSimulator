use crate::{LabResult, gmres::Krylov};
use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use std::{
    sync::{Arc, mpsc},
    time::Instant,
};
use wgpu::util::DeviceExt;

#[derive(Clone, Serialize)]
pub struct AdapterRecord {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub vendor: u32,
    pub device: u32,
    pub software: bool,
}

pub struct Context {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: AdapterRecord,
}

impl Context {
    pub async fn new(allow_software: bool) -> LabResult<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await?;
        let a = adapter.get_info();
        let software = a.device_type == wgpu::DeviceType::Cpu;
        let info = AdapterRecord {
            name: a.name,
            backend: format!("{:?}", a.backend),
            device_type: format!("{:?}", a.device_type),
            driver: a.driver,
            driver_info: a.driver_info,
            vendor: a.vendor,
            device: a.device,
            software,
        };
        if software && !allow_software {
            return Err(format!(
                "software adapter rejected: {}; use --allow-software only for functional checks",
                info.name
            )
            .into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("blood-stokes-lab"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await?;
        Ok(Self {
            device,
            queue,
            info,
        })
    }
}

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
    pad0: u32,
    pad1: u32,
}

#[derive(Default, Serialize)]
pub struct Counters {
    pub dispatches: u64,
    pub readbacks: u64,
    pub readback_bytes: u64,
    pub readback_wait_ms: f64,
}

pub struct System {
    context: Arc<Context>,
    n: usize,
    norm_slot: u32,
    basis_offset: u32,
    rhs_norm: f64,
    work: wgpu::Buffer,
    coefficients: wgpu::Buffer,
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipelines: Vec<wgpu::ComputePipeline>,
    pub counters: Counters,
}

impl System {
    pub fn new(
        context: Arc<Context>,
        ptr: &[u32],
        columns: &[u32],
        values: &[f32],
        restart: usize,
    ) -> LabResult<Self> {
        if restart == 0 || restart > 64 || values.iter().any(|v| !v.is_finite()) {
            return Err("invalid restart or f32 matrix values".into());
        }
        let n = ptr.len() - 1;
        let chunks = n.div_ceil(256);
        let device = &context.device;
        let storage = (n * (restart + 4) * 4) as u64;
        if storage > device.limits().max_storage_buffer_binding_size as u64 {
            return Err("Krylov workspace exceeds device storage buffer limit".into());
        }
        let init = |label, data: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        let row = init("row-ptr", bytemuck::cast_slice(ptr));
        let col = init("columns", bytemuck::cast_slice(columns));
        let val = init("values", bytemuck::cast_slice(values));
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let work = buffer(
            "vectors",
            storage,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        );
        let partial = buffer(
            "partial-dots",
            ((restart + 1) * chunks * 4) as u64,
            wgpu::BufferUsages::STORAGE,
        );
        let coefficients = buffer(
            "small-coefficients",
            (2 * (restart + 2) * 4) as u64,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        );
        let params = buffer(
            "params",
            std::mem::size_of::<Params>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lab-layout"),
            entries: &entries,
        });
        let buffers = [&row, &col, &val, &work, &partial, &coefficients, &params];
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
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
            label: Some("stokes-kernels"),
            source: wgpu::ShaderSource::Wgsl(include_str!("legacy.wgsl").into()),
        });
        let pipelines = ["spmv", "project", "reduce", "vector"]
            .iter()
            .map(|entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .collect();
        Ok(Self {
            context,
            n,
            norm_slot: (restart + 1) as u32,
            basis_offset: (3 * n) as u32,
            rhs_norm: 0.0,
            work,
            coefficients,
            params,
            group,
            pipelines,
            counters: Counters::default(),
        })
    }

    fn config(&self) -> Params {
        Params {
            n: self.n as u32,
            basis: self.basis_offset,
            chunks: self.n.div_ceil(256) as u32,
            aux: self.norm_slot,
            ..Default::default()
        }
    }

    // Every write is followed by its submission. Do not batch submissions while
    // reusing this uniform buffer: later parameter writes would change earlier work.
    fn dispatch(&mut self, pipeline: usize, p: Params, x: u32, y: u32) {
        self.context
            .queue
            .write_buffer(&self.params, 0, bytemuck::bytes_of(&p));
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipelines[pipeline]);
            pass.set_bind_group(0, &self.group, &[]);
            pass.dispatch_workgroups(x, y, 1);
        }
        self.context.queue.submit([encoder.finish()]);
        self.counters.dispatches += 1;
    }

    fn dots(&mut self, mut p: Params) {
        self.dispatch(1, p, p.chunks, p.count);
        p.src = 0;
        self.dispatch(2, p, p.count, 1);
    }

    fn read(&mut self, coefficients: bool, offset: u64, size: u64) -> LabResult<Vec<f32>> {
        let start = Instant::now();
        let staging = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(
            if coefficients {
                &self.coefficients
            } else {
                &self.work
            },
            offset,
            &staging,
            0,
            size,
        );
        self.context.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::sync_channel(1);
        staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.context
            .device
            .poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let data = staging.slice(..).get_mapped_range();
        let result = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        staging.unmap();
        self.counters.readbacks += 1;
        self.counters.readback_bytes += size;
        self.counters.readback_wait_ms += start.elapsed().as_secs_f64() * 1000.0;
        Ok(result)
    }

    pub fn reset(&mut self, rhs: &[f32]) -> LabResult<()> {
        if rhs.len() != self.n || rhs.iter().any(|v| !v.is_finite()) {
            return Err("invalid f32 RHS".into());
        }
        self.context.queue.write_buffer(
            &self.work,
            0,
            bytemuck::cast_slice(&vec![0.0_f32; self.n]),
        );
        self.context
            .queue
            .write_buffer(&self.work, (self.n * 4) as u64, bytemuck::cast_slice(rhs));
        self.rhs_norm = rhs.iter().map(|&v| (v as f64).powi(2)).sum::<f64>().sqrt();
        self.counters = Counters::default();
        Ok(())
    }

    pub fn solution(&mut self) -> LabResult<Vec<f64>> {
        Ok(self
            .read(false, 0, (self.n * 4) as u64)?
            .into_iter()
            .map(f64::from)
            .collect())
    }
}

impl Krylov for System {
    fn rhs_norm(&self) -> f64 {
        self.rhs_norm
    }
    fn start_cycle(&mut self) -> Result<f64, String> {
        let mut p = Params {
            src: 0,
            dst: (2 * self.n) as u32,
            mode: 1,
            aux: self.n as u32,
            ..self.config()
        };
        self.dispatch(0, p, p.chunks, 1);
        p = Params {
            src: (2 * self.n) as u32,
            count: 1,
            mode: 1,
            ..self.config()
        };
        self.dots(p);
        let copy = Params {
            src: p.src,
            dst: self.basis_offset,
            mode: 3,
            ..self.config()
        };
        self.dispatch(3, copy, copy.chunks, 1);
        Ok(self
            .read(true, (self.norm_slot * 4) as u64, 4)
            .map_err(|e| e.to_string())?[0] as f64)
    }
    fn arnoldi(&mut self, j: usize) -> Result<Vec<f64>, String> {
        let mut p = Params {
            src: self.basis_offset + (j * self.n) as u32,
            dst: (2 * self.n) as u32,
            ..self.config()
        };
        self.dispatch(0, p, p.chunks, 1);
        for pass in 0..2 {
            p = Params {
                src: (2 * self.n) as u32,
                count: (j + 1) as u32,
                add: pass,
                ..self.config()
            };
            // Second pass needs a separate coefficient range: subtracting the
            // accumulated h twice would corrupt w. Add h only after subtraction.
            self.dots(p);
            if pass == 1 {
                // See shader vector mode 4, which consumes the fresh second-pass dots.
                p.mode = 4;
            } else {
                p.mode = 1;
            }
            p.dst = p.src;
            self.dispatch(3, p, p.chunks, 1);
        }
        p = Params {
            src: (2 * self.n) as u32,
            count: 1,
            mode: 1,
            ..self.config()
        };
        self.dots(p);
        let copy = Params {
            src: p.src,
            dst: self.basis_offset + ((j + 1) * self.n) as u32,
            mode: 3,
            ..self.config()
        };
        self.dispatch(3, copy, copy.chunks, 1);
        let data = self
            .read(true, 0, ((self.norm_slot + 1) * 4) as u64)
            .map_err(|e| e.to_string())?;
        let mut h: Vec<_> = data[..j + 1].iter().map(|&v| v as f64).collect();
        h.push(data[self.norm_slot as usize] as f64);
        Ok(h)
    }
    fn update(&mut self, y: &[f64]) -> Result<(), String> {
        let y: Vec<f32> = y.iter().map(|&v| v as f32).collect();
        self.context
            .queue
            .write_buffer(&self.coefficients, 0, bytemuck::cast_slice(&y));
        let p = Params {
            dst: 0,
            count: y.len() as u32,
            mode: 2,
            ..self.config()
        };
        self.dispatch(3, p, p.chunks, 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wgsl_parses_and_validates_without_gpu() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("legacy.wgsl")).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
}
