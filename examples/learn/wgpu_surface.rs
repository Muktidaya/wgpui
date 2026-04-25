use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use wgpui::AppContext;
use wgpui::Styled;
/// Example: WgpuSurface with secondary render thread
/// Demonstrates using the WgpuSurface element with a dedicated render thread
use wgpui::{
    App, Application, Context, Render, WgpuSurfaceHandle, Window, WindowOptions, div, prelude::*,
    rgb, wgpu_surface,
};

// utilities for our cube vertex format
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[derive(Clone)]
struct CubeResources {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vert_buf: wgpu::Buffer,
    vertex_count: u32,
}

struct SurfaceExample {
    surface: WgpuSurfaceHandle,
    fps_rx: std::sync::mpsc::Receiver<f64>,
    display_fps: f64,
}

impl Render for SurfaceExample {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // pull any pending fps samples from channel
        while let Ok(f) = self.fps_rx.try_recv() {
            self.display_fps = f;
        }
        // ensure we keep repainting (needed since updates arrive off-thread)
        window.request_animation_frame();

        // The surface element will display the front buffer
        // Overlay a debug border and label for visibility
        div()
            .w(wgpui::px(1720.0))
            .h(wgpui::px(1080.0))
            .border_4()
            .border_color(rgb(0xff00ff))
            .rounded_lg()
            .shadow_xl()
            .bg(rgb(0x151a29))
            .m(wgpui::px(8.0))
            .child(
                wgpu_surface(self.surface.clone()).absolute().inset_0(), // Fill parent div
            )
            .child(
                div()
                    .absolute()
                    .top(wgpui::px(4.0))
                    .left(wgpui::px(8.0))
                    .text_color(rgb(0xff00ff))
                    .text_xl()
                    .child(format!("FPS: {:.1}", self.display_fps)),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        // Open a window
        _ = cx.open_window(WindowOptions::default(), |window: &mut Window, cx: &mut App| {
            // Create a WgpuSurfaceHandle (400x300 RGBA8)
            let surface = window.create_wgpu_surface(1720, 1080, wgpu::TextureFormat::Rgba8UnormSrgb)
                .expect("WgpuSurface not supported on this platform");
            let surface_thread = surface.clone();
            let fps_data: Arc<Mutex<f64>> = Arc::new(Mutex::new(0.0));
            let (fps_tx, fps_rx) = std::sync::mpsc::channel::<f64>();

            // secondary render thread
            let fps_shared = fps_data.clone();
            thread::spawn(move || {
                let mut frame: u32 = 0;
                // Wait for surface to be ready
                loop {
                    if surface_thread.back_buffer_view().is_some() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                // high‑performance render loop without sleeps or per‑frame printouts
                let mut last_report = std::time::Instant::now();
                let mut frame_count: u32 = 0;
                loop {
                    // throttle producer: wait until the compositor consumes last frame.
                    surface_thread.wait_for_present();
                    // draw to back buffer
                    let device = surface_thread.device();
                    let queue = surface_thread.queue();
                    // atomically grab view and its current size to avoid races on
                    // concurrent resizes (see handle.back_view_with_size doc comment).
                    let (view, (dw, dh)) = match surface_thread.back_view_with_size() {
                        Some(tuple) => tuple,
                        None => {
                            frame = frame.wrapping_add(1);
                            thread::sleep(Duration::from_nanos(500));
                            continue;
                        }
                    };

                    // --- GPU cube: spinning, facelit cube ---
                    use wgpu::util::DeviceExt;
                    thread_local! {
                        static RESOURCES: std::cell::RefCell<Option<CubeResources>> = std::cell::RefCell::new(None);
                    }

                    let t = frame as f32 * 0.01;
                    let (pipeline, uniform_buf, bind_group, vert_buf, vertex_count) =
                        RESOURCES.with(|r| {
                            let mut r = r.borrow_mut();
                            if r.is_none() {
                                // shader and pipeline setup
                                let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                                    label: Some("CubeShader"),
                                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@group(0) @binding(0) var<uniform> time: f32;

fn rotateY(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x * c + p.z * s, p.y, -p.x * s + p.z * c);
}
fn rotateX(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x, p.y * c - p.z * s, p.y * s + p.z * c);
}

@vertex
fn vs_main(in: VertexInput) -> VSOut {
    var pos = in.position;
    pos = rotateY(pos, time);
    pos = rotateX(pos, time * 0.5);
    let view = pos + vec3<f32>(0.0, 0.0, -4.0);
    let aspect = 400.0 / 300.0;
    let fovy = 45.0 * 3.14159265 / 180.0;
    let f = 1.0 / tan(fovy * 0.5);
    let znear = 0.1;
    let zfar = 100.0;
    let proj = mat4x4<f32>(
        vec4<f32>(f / aspect, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, f, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, (zfar + znear) / (znear - zfar), -1.0),
        vec4<f32>(0.0, 0.0, (2.0 * zfar * znear) / (znear - zfar), 0.0),
    );
    var out: VSOut;
    out.pos = proj * vec4<f32>(view, 1.0);
    var normal = in.normal;
    normal = rotateY(normal, time);
    normal = rotateX(normal, time * 0.5);
    out.normal = normal;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n = normalize(in.normal);
    let diff = max(dot(n, light), 0.0);
    return vec4<f32>(in.color * diff, 1.0);
}
"#)),
                                });

                                let bind_group_layout =
                                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                                        label: Some("CubeBGL"),
                                        entries: &[wgpu::BindGroupLayoutEntry {
                                            binding: 0,
                                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                                            ty: wgpu::BindingType::Buffer {
                                                ty: wgpu::BufferBindingType::Uniform,
                                                has_dynamic_offset: false,
                                                min_binding_size: None,
                                            },
                                            count: None,
                                        }],
                                    });
                                let pipeline_layout =
                                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                        label: Some("CubePipelineLayout"),
                                        bind_group_layouts: &[&bind_group_layout],
                                        immediate_size: 0,
                                    });
                                let pipeline =
                                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                        label: Some("CubePipeline"),
                                        layout: Some(&pipeline_layout),
                                        vertex: wgpu::VertexState {
                                            module: &shader,
                                            entry_point: Some("vs_main"),
                                            buffers: &[wgpu::VertexBufferLayout {
                                                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                                                step_mode: wgpu::VertexStepMode::Vertex,
                                                attributes: &[
                                                    wgpu::VertexAttribute {
                                                        format: wgpu::VertexFormat::Float32x3,
                                                        offset: 0,
                                                        shader_location: 0,
                                                    },
                                                    wgpu::VertexAttribute {
                                                        format: wgpu::VertexFormat::Float32x3,
                                                        offset: 4 * 3,
                                                        shader_location: 1,
                                                    },
                                                    wgpu::VertexAttribute {
                                                        format: wgpu::VertexFormat::Float32x3,
                                                        offset: 4 * 6,
                                                        shader_location: 2,
                                                    },
                                                ],
                                            }],
                                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                                        },
                                        fragment: Some(wgpu::FragmentState {
                                            module: &shader,
                                            entry_point: Some("fs_main"),
                                            targets: &[Some(wgpu::ColorTargetState {
                                                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                                blend: Some(wgpu::BlendState::REPLACE),
                                                write_mask: wgpu::ColorWrites::ALL,
                                            })],
                                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                                        }),
                                        primitive: wgpu::PrimitiveState {
                                            topology: wgpu::PrimitiveTopology::TriangleList,
                                            ..Default::default()
                                        },
                                        depth_stencil: Some(wgpu::DepthStencilState {
                                            format: wgpu::TextureFormat::Depth24Plus,
                                            depth_write_enabled: true,
                                            depth_compare: wgpu::CompareFunction::Less,
                                            stencil: wgpu::StencilState::default(),
                                            bias: wgpu::DepthBiasState::default(),
                                        }),
                                        multisample: wgpu::MultisampleState::default(),
                                        multiview_mask: None,
                                        cache: None,
                                    });

                                let vertices: [Vertex; 36] = [
                                    Vertex{position:[-1.0,-1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[1.0,-1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[1.0,1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[-1.0,-1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[1.0,1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[-1.0,1.0,1.0],normal:[0.0,0.0,1.0],color:[1.0,0.0,0.0]},
                                    Vertex{position:[1.0,-1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[-1.0,-1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[-1.0,1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[1.0,-1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[-1.0,1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[1.0,1.0,-1.0],normal:[0.0,0.0,-1.0],color:[0.0,1.0,0.0]},
                                    Vertex{position:[-1.0,1.0,1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,-1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[-1.0,1.0,1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,-1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[-1.0,1.0,-1.0],normal:[0.0,1.0,0.0],color:[0.0,0.0,1.0]},
                                    Vertex{position:[-1.0,-1.0,-1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[1.0,-1.0,-1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[1.0,-1.0,1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[-1.0,-1.0,-1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[1.0,-1.0,1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[-1.0,-1.0,1.0],normal:[0.0,-1.0,0.0],color:[1.0,1.0,0.0]},
                                    Vertex{position:[1.0,-1.0,1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[1.0,-1.0,-1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,-1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[1.0,-1.0,1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,-1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[1.0,1.0,1.0],normal:[1.0,0.0,0.0],color:[1.0,0.0,1.0]},
                                    Vertex{position:[-1.0,-1.0,-1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                    Vertex{position:[-1.0,-1.0,1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                    Vertex{position:[-1.0,1.0,1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                    Vertex{position:[-1.0,-1.0,-1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                    Vertex{position:[-1.0,1.0,1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                    Vertex{position:[-1.0,1.0,-1.0],normal:[-1.0,0.0,0.0],color:[0.0,1.0,1.0]},
                                ];
                                let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
                                    label: Some("CubeVertexBuf"),
                                    contents: bytemuck::cast_slice(&vertices),
                                    usage: wgpu::BufferUsages::VERTEX,
                                });
                                let vertex_count = vertices.len() as u32;

                                // depth texture for 3D ordering
                                let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
                                    label: Some("CubeDepth"),
                                    size: wgpu::Extent3d { width: 400, height: 300, depth_or_array_layers: 1 },
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: wgpu::TextureFormat::Depth24Plus,
                                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                    view_formats: &[],
                                });
                                let _depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

                                let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
                                    label: Some("CubeUniformBuf"),
                                    contents: bytemuck::cast_slice(&[0f32]),
                                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                                });
                                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
                                    label: Some("CubeBindGroup"),
                                    layout: &bind_group_layout,
                                    entries: &[wgpu::BindGroupEntry{
                                        binding: 0,
                                        resource: uniform_buf.as_entire_binding(),
                                    }],
                                });

                                *r = Some(CubeResources{
                                    pipeline: pipeline.clone(),
                                    uniform_buf: uniform_buf.clone(),
                                    bind_group: bind_group.clone(),
                                    vert_buf: vertex_buf.clone(),
                                    vertex_count,
                                });
                            }
                            let res = r.as_ref().unwrap();
                            (res.pipeline.clone(), res.uniform_buf.clone(), res.bind_group.clone(), res.vert_buf.clone(), res.vertex_count)
                        });

                    queue.write_buffer(&uniform_buf, 0, bytemuck::cast_slice(&[t]));

                    // depth texture/view sized to match the returned view dimensions
                    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("CubeDepth"),
                        size: wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth24Plus,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    });
                    let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor{
                        label: Some("SurfaceExample Encoder"),
                    });
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                            label: Some("SurfaceExample Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment{
                                view: &view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations{
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment{
                                view: &depth_view,
                                depth_ops: Some(wgpu::Operations{
                                    load: wgpu::LoadOp::Clear(1.0),
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }),
                            multiview_mask: None,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                        });
                        rpass.set_pipeline(&pipeline);
                        rpass.set_bind_group(0, &bind_group, &[]);
                        rpass.set_vertex_buffer(0, vert_buf.slice(..));
                        rpass.draw(0..vertex_count, 0..1);
                    }
                    let _ = queue.submit(Some(encoder.finish()));                    surface_thread.present();
                    frame = frame.wrapping_add(1);

                    // update frame counter and report once per second
                    frame_count = frame_count.wrapping_add(1);
                    let now = std::time::Instant::now();
                    if now.duration_since(last_report) >= Duration::from_secs(1) {
                        let fps = frame_count as f64; // frames per second
                        *fps_shared.lock().unwrap() = fps;
                        frame_count = 0;
                        last_report = now;
                    }
                }
            });

            // construct entity and keep handle in outer scope
            let handle = cx.new(|_cx| SurfaceExample { surface, fps_rx, display_fps: 0.0 });
            // timer thread: wake once per second and push fps into channel
            let fps_shared = fps_data.clone();
            let tx_clone = fps_tx.clone();
            thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let val = *fps_shared.lock().unwrap();
                    let _ = tx_clone.send(val);
                }
            });
            handle
        });
    });
}
