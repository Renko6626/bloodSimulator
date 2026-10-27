use crate::model::{Config, Field};
use bytemuck::{Pod, Zeroable};
use serde_json::{Value, json};
use std::{sync::mpsc, time::Instant};
use wgpu::util::DeviceExt;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    nx: u32,
    ny: u32,
    periodic_y: u32,
    omega: f32,
    force: [f32; 2],
    scale: f32,
    pad: f32,
}
pub struct Context {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: Value,
    pub timestamps: bool,
}
impl Context {
    pub fn new(allow_software: bool) -> Result<Self> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
                ..Default::default()
            });
            let a = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: None,
                })
                .await?;
            let i = a.get_info();
            let software = i.device_type == wgpu::DeviceType::Cpu;
            if software && !allow_software {
                return Err("software adapter rejected for GPU measurement".into());
            }
            let features = a.features() & wgpu::Features::TIMESTAMP_QUERY;
            let (device, queue) = a
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("lbm-mrt-lab"),
                    required_features: features,
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                })
                .await?;
            Ok(Self {
                device,
                queue,
                timestamps: !features.is_empty(),
                info: json!({"name":i.name,"backend":format!("{:?}",i.backend),"type":format!("{:?}",i.device_type),"driver":i.driver,"driver_info":i.driver_info,"software":software}),
            })
        })
    }
}
#[derive(Default, Clone, Copy)]
pub struct Times {
    pub upload: f64,
    pub encode: f64,
    pub submit: f64,
    pub wait: f64,
    pub decode: f64,
    pub gpu: f64,
    pub whole: f64,
}
impl std::ops::AddAssign for Times {
    fn add_assign(&mut self, b: Self) {
        self.upload += b.upload;
        self.encode += b.encode;
        self.submit += b.submit;
        self.wait += b.wait;
        self.decode += b.decode;
        self.gpu += b.gpu;
        self.whole += b.whole;
    }
}
pub struct Gpu<'a> {
    pub ctx: &'a Context,
    pub c: Config,
    _buffers: [wgpu::Buffer; 2],
    uniform: wgpu::Buffer,
    kicks: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    groups: [wgpu::BindGroup; 2],
    collide: wgpu::ComputePipeline,
    kick: wgpu::ComputePipeline,
    macros: wgpu::ComputePipeline,
    queries: Option<wgpu::QuerySet>,
    resolve: Option<wgpu::Buffer>,
    current: usize,
    stride: usize,
    params: Vec<u8>,
}
impl<'a> Gpu<'a> {
    pub fn new(ctx: &'a Context, c: Config, initial: &[f64]) -> Result<Self> {
        let d = &ctx.device;
        let n = c.len();
        assert_eq!(initial.len(), 9 * n);
        let floats: Vec<f32> = initial.iter().map(|x| *x as f32).collect();
        let init = |label, data: &[u8], usage| {
            d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage,
            })
        };
        let pop_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let buffers = [
            init("populations-a", bytemuck::cast_slice(&floats), pop_usage),
            init(
                "populations-b",
                bytemuck::cast_slice(&vec![0_f32; 9 * n]),
                pop_usage,
            ),
        ];
        let stride = d.limits().min_uniform_buffer_offset_alignment as usize;
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("step-parameters"),
            size: ((c.substeps + 1) * stride) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let kicks = init(
            "momentum-kicks",
            bytemuck::cast_slice(&vec![[0_f32; 2]; n]),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let output = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("macroscopic-field"),
            size: (n * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("persistent-readback"),
            size: (n * 16 + 16) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entries: Vec<_> = (0..5)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 0 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding == 4,
                        }
                    },
                    has_dynamic_offset: binding == 0,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lbm-layout"),
            entries: &entries,
        });
        let group = |current: usize| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lbm-ping-pong"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform,
                            offset: 0,
                            size: std::num::NonZeroU64::new(32),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffers[current].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffers[1 - current].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: kicks.as_entire_binding(),
                    },
                ],
            })
        };
        let groups = [group(0), group(1)];
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lbm-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mrt-guo"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lbm.wgsl").into()),
        });
        let pipe = |name| {
            d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let queries = ctx.timestamps.then(|| {
            d.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("lbm-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            })
        });
        let resolve = ctx.timestamps.then(|| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        Ok(Self {
            ctx,
            c,
            _buffers: buffers,
            uniform,
            kicks,
            output,
            readback,
            groups,
            collide: pipe("collide_stream"),
            kick: pipe("kick"),
            macros: pipe("macros"),
            queries,
            resolve,
            current: 0,
            stride,
            params: vec![0; (c.substeps + 1) * stride],
        })
    }
    pub fn advance(
        &mut self,
        accelerations: &[[f64; 2]],
        end_force: [f64; 2],
        kicks: Option<&[[f32; 2]]>,
    ) -> Result<(Field, Times)> {
        assert!(accelerations.len() <= self.c.substeps);
        let whole = Instant::now();
        let mut times = Times::default();
        let start = Instant::now();
        let c = self.c;
        for (i, &a) in accelerations
            .iter()
            .chain(std::iter::once(&end_force))
            .enumerate()
        {
            let f = c.force(a);
            let p = Params {
                nx: c.nx as u32,
                ny: c.ny as u32,
                periodic_y: c.periodic_y as u32,
                omega: (1. / c.tau()) as f32,
                force: [f[0] as f32, f[1] as f32],
                scale: (c.h / c.dt) as f32,
                pad: 0.,
            };
            self.params[i * self.stride..i * self.stride + 32]
                .copy_from_slice(bytemuck::bytes_of(&p));
        }
        self.ctx.queue.write_buffer(
            &self.uniform,
            0,
            &self.params[..(accelerations.len() + 1) * self.stride],
        );
        if let Some(k) = kicks {
            assert_eq!(k.len(), c.len());
            self.ctx
                .queue
                .write_buffer(&self.kicks, 0, bytemuck::cast_slice(k));
        }
        times.upload = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lbm-mechanical-substep"),
            });
        {
            let timestamp_writes =
                self.queries
                    .as_ref()
                    .map(|q| wgpu::ComputePassTimestampWrites {
                        query_set: q,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    });
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("all-lattice-steps"),
                timestamp_writes,
            });
            let groups = c.len().div_ceil(128) as u32;
            if kicks.is_some() {
                pass.set_pipeline(&self.kick);
                pass.set_bind_group(0, &self.groups[self.current], &[0]);
                pass.dispatch_workgroups(groups, 1, 1);
            }
            pass.set_pipeline(&self.collide);
            for i in 0..accelerations.len() {
                pass.set_bind_group(0, &self.groups[self.current], &[(i * self.stride) as u32]);
                pass.dispatch_workgroups(groups, 1, 1);
                self.current = 1 - self.current;
            }
            pass.set_pipeline(&self.macros);
            pass.set_bind_group(
                0,
                &self.groups[self.current],
                &[(accelerations.len() * self.stride) as u32],
            );
            pass.dispatch_workgroups(groups, 1, 1);
        }
        let bytes = (c.len() * 16) as u64;
        enc.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, bytes);
        if let (Some(q), Some(b)) = (&self.queries, &self.resolve) {
            enc.resolve_query_set(q, 0..2, b, 0);
            enc.copy_buffer_to_buffer(b, 0, &self.readback, bytes, 16);
        }
        let cmd = enc.finish();
        times.encode = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        self.ctx.queue.submit([cmd]);
        times.submit = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        self.ctx.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        times.wait = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let data = self.readback.slice(..).get_mapped_range();
        let vals: &[[f32; 4]] = bytemuck::cast_slice(&data[..bytes as usize]);
        let samples = vals
            .iter()
            .map(|p| [1. + p[3] as f64, p[1] as f64, p[2] as f64])
            .collect();
        if self.queries.is_some() {
            let stamps: &[u64] = bytemuck::cast_slice(&data[bytes as usize..bytes as usize + 16]);
            times.gpu = stamps[1].saturating_sub(stamps[0]) as f64
                * self.ctx.queue.get_timestamp_period() as f64
                * 1e-6;
        }
        drop(data);
        self.readback.unmap();
        times.decode = start.elapsed().as_secs_f64() * 1000.;
        times.whole = whole.elapsed().as_secs_f64() * 1000.;
        Ok((Field { samples }, times))
    }
}
