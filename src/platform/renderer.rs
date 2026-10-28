use std::sync::Arc;

use crate::{
    AtlasTextureId, AtlasTile, DevicePixels, FrameCapture, FrameCaptureError, GpuSpecs, Hsla,
    LinearColorStop, MonochromeSprite, PlatformAtlas, PrimitiveBatch, Quad, ScaledPixels, Scene,
    TransformationMatrix, color, geometry,
    platform::{atlas::WgpuAtlas, render_context::WgpuContext},
};
use futures::channel::oneshot;

#[allow(dead_code)]
const fn map_attributes<const N: usize>(
    attribs: &'static [wgpu::VertexAttribute; N],
    location_offset: u32,
    offset_offset: wgpu::BufferAddress,
) -> [wgpu::VertexAttribute; N] {
    let mut result = [wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        // NOTE(mdeand): Dummy format, will be overwritten.
        format: wgpu::VertexFormat::Uint8x2,
    }; N];
    let mut i = 0;

    while i < result.len() {
        result[i] = wgpu::VertexAttribute {
            offset: attribs[i].offset + offset_offset,
            shader_location: attribs[i].shader_location + location_offset,
            format: attribs[i].format,
        };
        i += 1;
    }

    result
}

impl color::Hsla {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 4] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(color::Hsla, h) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(color::Hsla, s) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(color::Hsla, l) as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(color::Hsla, a) as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32,
        },
    ];
}

impl color::LinearColorStop {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 2] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(LinearColorStop, color) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(LinearColorStop, percentage) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32,
        },
    ];
}

impl color::Background {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 7] = &{
        let linear_color_stop_vertex_attributes = map_attributes(
            LinearColorStop::VERTEX_ATTRIBUTES,
            4,
            std::mem::offset_of!(color::Background, colors) as wgpu::BufferAddress,
        );

        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(color::Background, tag) as wgpu::BufferAddress,
                shader_location: 0,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(color::Background, color_space) as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(color::Background, solid) as wgpu::BufferAddress,
                shader_location: 2,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(color::Background, gradient_angle_or_pattern_height)
                    as wgpu::BufferAddress,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32,
            },
            linear_color_stop_vertex_attributes[0],
            linear_color_stop_vertex_attributes[1],
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(color::Background, pad) as wgpu::BufferAddress,
                shader_location: 6,
                format: wgpu::VertexFormat::Uint32,
            },
        ]
    };
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalParams {
    viewport_size: [f32; 2],
    premultimated_alpha: u32,
    pad: u32,
}

impl GlobalParams {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 3] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GlobalParams, viewport_size) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GlobalParams, premultimated_alpha) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Uint32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GlobalParams, pad) as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Uint32,
        },
    ];
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Bounds {
    origin: [f32; 2],
    size: [f32; 2],
}

impl geometry::Corners<ScaledPixels> {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 4] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Corners<ScaledPixels>, top_left)
                as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Corners<ScaledPixels>, top_right)
                as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Corners<ScaledPixels>, bottom_right)
                as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Corners<ScaledPixels>, bottom_left)
                as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32,
        },
    ];
}

impl geometry::Edges<ScaledPixels> {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 4] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Edges<ScaledPixels>, top) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Edges<ScaledPixels>, right)
                as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Edges<ScaledPixels>, bottom)
                as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(geometry::Edges<ScaledPixels>, left)
                as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32,
        },
    ];
}

impl Bounds {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 2] = &[
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(Bounds, origin) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(Bounds, size) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
    ];
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SurfaceParams {
    bounds: Bounds,
    content_mask: Bounds,
}

impl Quad {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 22] = &{
        let bounds_vertex_attributes = map_attributes(
            Bounds::VERTEX_ATTRIBUTES,
            2,
            std::mem::offset_of!(Quad, bounds) as wgpu::BufferAddress,
        );

        let content_mask_vertex_attributes = map_attributes(
            Bounds::VERTEX_ATTRIBUTES,
            4,
            std::mem::offset_of!(Quad, content_mask) as wgpu::BufferAddress,
        );

        let background_vertex_attributes = map_attributes(
            color::Background::VERTEX_ATTRIBUTES,
            6,
            std::mem::offset_of!(Quad, background) as wgpu::BufferAddress,
        );

        let border_color_vertex_attributes = map_attributes(
            color::Hsla::VERTEX_ATTRIBUTES,
            11,
            std::mem::offset_of!(Quad, border_color) as wgpu::BufferAddress,
        );

        let corner_radii_vertex_attributes = map_attributes(
            geometry::Corners::<ScaledPixels>::VERTEX_ATTRIBUTES,
            15,
            std::mem::offset_of!(Quad, corner_radii) as wgpu::BufferAddress,
        );

        let border_widths_vertex_attributes = map_attributes(
            geometry::Edges::<ScaledPixels>::VERTEX_ATTRIBUTES,
            19,
            std::mem::offset_of!(Quad, border_widths) as wgpu::BufferAddress,
        );

        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(Quad, order) as wgpu::BufferAddress,
                shader_location: 0,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(Quad, border_style) as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Uint32,
            },
            bounds_vertex_attributes[0],
            bounds_vertex_attributes[1],
            content_mask_vertex_attributes[0],
            content_mask_vertex_attributes[1],
            background_vertex_attributes[0],
            background_vertex_attributes[1],
            background_vertex_attributes[2],
            background_vertex_attributes[3],
            border_color_vertex_attributes[0],
            border_color_vertex_attributes[1],
            border_color_vertex_attributes[2],
            border_color_vertex_attributes[3],
            corner_radii_vertex_attributes[0],
            corner_radii_vertex_attributes[1],
            corner_radii_vertex_attributes[2],
            corner_radii_vertex_attributes[3],
            border_widths_vertex_attributes[0],
            border_widths_vertex_attributes[1],
            border_widths_vertex_attributes[2],
            border_widths_vertex_attributes[3],
        ]
    };
}

#[repr(C)]
#[allow(dead_code)]
struct QuadsData {
    globals: GlobalParams,
}

#[repr(C)]
#[allow(dead_code)]
struct ShadowsData {
    globals: GlobalParams,
}

#[repr(C)]
#[allow(dead_code)]
struct PathRasterizationData {
    globals: GlobalParams,
}

#[allow(dead_code)]
struct PathsData {
    globals: GlobalParams,
    t_sprite: wgpu::TextureView,
    s_sprite: wgpu::Sampler,
}

#[allow(dead_code)]
struct UnderlinesData {
    globals: GlobalParams,
}

#[allow(dead_code)]
struct MonoSpritesData {
    globals: GlobalParams,
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    t_sprite: wgpu::TextureView,
    s_sprite: wgpu::Sampler,
}

#[allow(dead_code)]
struct PolySpritesData {
    globals: GlobalParams,
    t_sprite: wgpu::TextureView,
    s_sprite: wgpu::Sampler,
}

#[allow(dead_code)]
struct SurfacesData {
    globals: GlobalParams,
    surface_params: SurfaceParams,
    t_y: wgpu::TextureView,
    t_cb_cr: wgpu::TextureView,
    s_texture: wgpu::Sampler,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PathSprite {
    bounds: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PathRasterizationVertex {
    xy_position: [f32; 2],
    st_position: [f32; 2],
    color: color::Background,
    bounds: [f32; 4],
}

impl AtlasTextureId {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 2] = &{
        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasTextureId, index) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasTextureId, kind) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 1,
            },
        ]
    };
}

#[repr(C)]
#[allow(dead_code)]
struct AtlasBounds {
    origin: [i32; 2],
    size: [i32; 2],
}

impl AtlasBounds {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 2] = &{
        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasBounds, origin) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Sint32x2,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasBounds, size) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Sint32x2,
                shader_location: 1,
            },
        ]
    };
}

impl AtlasTile {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 6] = &{
        let texture_id_vertex_attributes = map_attributes(
            AtlasTextureId::VERTEX_ATTRIBUTES,
            0,
            std::mem::offset_of!(AtlasTile, texture_id) as wgpu::BufferAddress,
        );

        let bounds_vertex_attributes = map_attributes(
            AtlasBounds::VERTEX_ATTRIBUTES,
            4,
            std::mem::offset_of!(AtlasTile, bounds) as wgpu::BufferAddress,
        );

        [
            texture_id_vertex_attributes[0],
            texture_id_vertex_attributes[1],
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasTile, tile_id) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(AtlasTile, padding) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 3,
            },
            bounds_vertex_attributes[0],
            bounds_vertex_attributes[1],
        ]
    };
}

impl TransformationMatrix {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 2] = &{
        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(TransformationMatrix, rotation_scale)
                    as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Float32x4,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(TransformationMatrix, translation)
                    as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Float32x2,
                shader_location: 1,
            },
        ]
    };
}

impl MonochromeSprite {
    #[allow(dead_code)]
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute; 16] = &{
        let bounds_vertex_attributes = map_attributes(
            Bounds::VERTEX_ATTRIBUTES,
            2,
            std::mem::offset_of!(MonochromeSprite, bounds) as wgpu::BufferAddress,
        );

        let content_mask_vertex_attributes = map_attributes(
            Bounds::VERTEX_ATTRIBUTES,
            4,
            std::mem::offset_of!(MonochromeSprite, content_mask) as wgpu::BufferAddress,
        );

        let color_vertex_attributes = map_attributes(
            Hsla::VERTEX_ATTRIBUTES,
            6,
            std::mem::offset_of!(MonochromeSprite, color) as wgpu::BufferAddress,
        );

        let tile_vertex_attributes = map_attributes(
            AtlasTile::VERTEX_ATTRIBUTES,
            8,
            std::mem::offset_of!(MonochromeSprite, tile) as wgpu::BufferAddress,
        );

        let transformation_matrix_vertex_attributes = map_attributes(
            TransformationMatrix::VERTEX_ATTRIBUTES,
            14,
            std::mem::offset_of!(MonochromeSprite, transformation) as wgpu::BufferAddress,
        );

        [
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(MonochromeSprite, order) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::offset_of!(MonochromeSprite, pad) as wgpu::BufferAddress,
                format: wgpu::VertexFormat::Uint32,
                shader_location: 1,
            },
            bounds_vertex_attributes[0],
            bounds_vertex_attributes[1],
            content_mask_vertex_attributes[0],
            content_mask_vertex_attributes[1],
            color_vertex_attributes[0],
            color_vertex_attributes[1],
            tile_vertex_attributes[0],
            tile_vertex_attributes[1],
            tile_vertex_attributes[2],
            tile_vertex_attributes[3],
            tile_vertex_attributes[4],
            tile_vertex_attributes[5],
            transformation_matrix_vertex_attributes[0],
            transformation_matrix_vertex_attributes[1],
        ]
    };
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ColorAdjustments {
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    _padding: [f32; 3],
}

struct WgpuPipelines {
    #[allow(dead_code)]
    color_targets: Vec<Option<wgpu::ColorTargetState>>,

    quads_bind_group_layout: wgpu::BindGroupLayout,
    shadows_bind_group_layout: wgpu::BindGroupLayout,
    underlines_bind_group_layout: wgpu::BindGroupLayout,
    sprites_bind_group_layout: wgpu::BindGroupLayout,
    mono_sprites_bind_group_layout: wgpu::BindGroupLayout,
    poly_sprites_bind_group_layout: wgpu::BindGroupLayout,
    surfaces_bind_group_layout: wgpu::BindGroupLayout,
    path_rasterization_bind_group_layout: wgpu::BindGroupLayout,
    path_sprites_bind_group_layout: wgpu::BindGroupLayout,

    globals_bind_group: wgpu::BindGroup,
    color_adjustments_bind_group: wgpu::BindGroup,

    quads_pipeline: wgpu::RenderPipeline,
    shadows_pipeline: wgpu::RenderPipeline,
    underlines_pipeline: wgpu::RenderPipeline,
    mono_sprites_pipeline: wgpu::RenderPipeline,
    poly_sprites_pipeline: wgpu::RenderPipeline,
    surfaces_pipeline: wgpu::RenderPipeline,
    path_rasterization_pipeline: wgpu::RenderPipeline,
    paths_pipeline: wgpu::RenderPipeline,
}

impl WgpuPipelines {
    pub fn new(
        context: &WgpuContext,
        surface_configuration: &wgpu::SurfaceConfiguration,
        _path_sample_count: u32,
    ) -> Self {
        let path_rasterization_shader =
            context
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("path_rasterization_shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        format!(
                            "{}\n{}",
                            include_str!("../shaders/path_common.wgsl"),
                            include_str!("../shaders/path_rasterization.wgsl")
                        )
                        .into(),
                    ),
                });

        let paths_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("paths_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    format!(
                        "{}\n{}",
                        include_str!("../shaders/path_common.wgsl"),
                        include_str!("../shaders/paths.wgsl")
                    )
                    .into(),
                ),
            });

        let quads_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("quads_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/quads.wgsl").into()),
            });

        let shadows_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shadows_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadows.wgsl").into()),
            });

        let underlines_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("underlines_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/underlines.wgsl").into()),
            });

        let mono_sprite_shader =
            context
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("mono_sprites shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("../shaders/mono_sprites.wgsl").into(),
                    ),
                });

        let poly_sprite_shader =
            context
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("poly_sprites shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("../shaders/poly_sprites.wgsl").into(),
                    ),
                });

        let blend_mode = match surface_configuration.alpha_mode {
            wgpu::CompositeAlphaMode::PreMultiplied => {
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
            }
            _ => wgpu::BlendState::ALPHA_BLENDING,
        };

        let color_targets = &[Some(wgpu::ColorTargetState {
            format: surface_configuration.format,
            blend: Some(blend_mode),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let globals_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("globals"),
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

        let color_adjustments_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("color_adjustments_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let sprites_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sprite_bind_group_layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let quads_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("quads_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let quads_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("quads_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&quads_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let shadows_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("shadows_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let shadows_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("shadows_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&shadows_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let underlines_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("underlines_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let underlines_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("underlines_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&underlines_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let mono_sprites_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Mono sprites bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let mono_sprites_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Mono sprites pipeline layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&color_adjustments_bind_group_layout),
                        Some(&sprites_bind_group_layout),
                        Some(&mono_sprites_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let poly_sprites_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Poly sprites bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let poly_sprites_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Poly sprites pipeline layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&sprites_bind_group_layout),
                        Some(&poly_sprites_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let surfaces_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("surfaces_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/surfaces.wgsl").into()),
            });

        let surfaces_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("surfaces_bind_group_layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let surfaces_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("surfaces_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&surfaces_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let path_rasterization_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("path_rasterization_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let path_sprites_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("path_sprites_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let path_rasterization_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("path_rasterization_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&path_rasterization_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let paths_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("paths_pipeline_layout"),
                    bind_group_layouts: &[
                        Some(&globals_bind_group_layout),
                        Some(&path_sprites_bind_group_layout),
                        Some(&sprites_bind_group_layout),
                    ],
                    immediate_size: 0,
                });

        let path_rasterization_targets = &[Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8Unorm,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let globals_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("globals_bind_group"),
                layout: &globals_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &context.globals_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        let color_adjustments_bind_group =
            context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("color_adjustments_bind_group"),
                    layout: &color_adjustments_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &context.color_adjustments_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        Self {
            color_targets: color_targets.to_vec(),

            quads_bind_group_layout,
            shadows_bind_group_layout,
            underlines_bind_group_layout,
            mono_sprites_bind_group_layout,
            sprites_bind_group_layout,
            poly_sprites_bind_group_layout,

            globals_bind_group,
            color_adjustments_bind_group,

            quads_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("quads"),
                    layout: Some(&quads_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &quads_shader,
                        entry_point: Some("vs_quad"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &quads_shader,
                        entry_point: Some("fs_quad"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            shadows_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("shadows"),
                    layout: Some(&shadows_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shadows_shader,
                        entry_point: Some("vs_shadow"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shadows_shader,
                        entry_point: Some("fs_shadow"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            underlines_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("underlines"),
                    layout: Some(&underlines_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &underlines_shader,
                        entry_point: Some("vs_underline"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &underlines_shader,
                        entry_point: Some("fs_underline"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            mono_sprites_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("mono_sprites"),
                    layout: Some(&mono_sprites_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &mono_sprite_shader,
                        entry_point: Some("vs_mono_sprite"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    fragment: Some(wgpu::FragmentState {
                        module: &mono_sprite_shader,
                        entry_point: Some("fs_mono_sprite"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            poly_sprites_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("poly_sprites"),
                    layout: Some(&poly_sprites_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &poly_sprite_shader,
                        entry_point: Some("vs_poly_sprite"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    fragment: Some(wgpu::FragmentState {
                        module: &poly_sprite_shader,
                        entry_point: Some("fs_poly_sprite"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            surfaces_bind_group_layout,

            surfaces_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("surfaces"),
                    layout: Some(&surfaces_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &surfaces_shader,
                        entry_point: Some("vs_surface"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    fragment: Some(wgpu::FragmentState {
                        module: &surfaces_shader,
                        entry_point: Some("fs_surface"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            path_rasterization_bind_group_layout,
            path_sprites_bind_group_layout,

            path_rasterization_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("path_rasterization"),
                    layout: Some(&path_rasterization_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &path_rasterization_shader,
                        entry_point: Some("vs_path_rasterization"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    fragment: Some(wgpu::FragmentState {
                        module: &path_rasterization_shader,
                        entry_point: Some("fs_path_rasterization"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: path_rasterization_targets,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ),

            paths_pipeline: context.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("paths"),
                    layout: Some(&paths_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &paths_shader,
                        entry_point: Some("vs_path"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    fragment: Some(wgpu::FragmentState {
                        module: &paths_shader,
                        entry_point: Some("fs_path"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: color_targets,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ),
        }
    }
}

struct RenderingParameters {
    #[allow(dead_code)]
    path_sample_count: u32,
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
}

impl RenderingParameters {
    fn from_env() -> Self {
        use std::env;

        let path_sample_count = env::var("WGPUI_PATH_SAMPLE_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let gamma = env::var("WGPUI_FONTS_GAMMA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.8_f32)
            .clamp(1.0, 2.2);
        let gamma_ratios = crate::platform::get_gamma_correction_ratios(gamma);
        let grayscale_enhanced_contrast = env::var("WGPUI_FONTS_GRAYSCALE_ENHANCED_CONTRAST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0_f32)
            .max(0.0);

        Self {
            path_sample_count,
            gamma_ratios,
            grayscale_enhanced_contrast,
        }
    }
}

use parking_lot::Mutex;
use std::collections::HashMap;

pub struct WgpuRenderer {
    context: Arc<WgpuContext>,
    surface: Option<wgpu::Surface<'static>>,
    offscreen_texture: Option<wgpu::Texture>,
    surface_configuration: wgpu::SurfaceConfiguration,
    atlas_sampler: wgpu::Sampler,
    surface_sampler: wgpu::Sampler,
    atlas: Arc<WgpuAtlas>,
    pipelines: WgpuPipelines,
    rendering_parameters: RenderingParameters,

    path_intermediate_texture: Option<wgpu::Texture>,
    path_intermediate_view: Option<wgpu::TextureView>,

    // Cache bind groups for each double-buffered surface texture generation.
    // Resizing preserves SurfaceId but replaces both texture views.
    surface_bind_groups:
        Mutex<HashMap<(crate::platform::surface_registry::SurfaceId, u64, usize), SurfaceBinding>>,
    frame_capture_requests: FrameCaptureRequests,
    configure_pending: std::sync::atomic::AtomicBool,
}

// Each placement needs its own uniform: queued writes precede every draw in the submission.
struct SurfaceBinding {
    params: wgpu::Buffer,
    bind_groups: [wgpu::BindGroup; 2],
}

/// Upper bound on waiting for one capture's copy to complete before the request is reported
/// as failed. The copy is a single blit of a single frame, so this is only ever reached when
/// the device itself has stopped making progress.
const CAPTURE_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// One pending capture of the next complete-scene frame.
struct FrameCaptureRequest {
    completion: oneshot::Sender<Result<FrameCapture, FrameCaptureError>>,
}

/// The one-shot slot holding at most one pending frame capture.
#[derive(Default)]
struct FrameCaptureRequests {
    pending: Mutex<Option<FrameCaptureRequest>>,
}

impl FrameCaptureRequests {
    /// Store `request`, returning the request it superseded, if any.
    fn install(&self, request: FrameCaptureRequest) -> Option<FrameCaptureRequest> {
        self.pending.lock().replace(request)
    }

    /// Take the pending request, leaving the slot empty: the next call returns `None`.
    fn take(&self) -> Option<FrameCaptureRequest> {
        self.pending.lock().take()
    }
}

/// A capture request bound to the frame it was taken for: the readback buffer that holds that
/// frame's copy, or the reason no copy could be encoded.
struct PendingFrameCapture {
    request: FrameCaptureRequest,
    readback: Result<FrameCaptureReadback, FrameCaptureError>,
}

impl PendingFrameCapture {
    /// Encode one copy of `texture` for `request` into this frame's command encoder.
    fn encode(
        request: FrameCaptureRequest,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let readback = encode_capture_copy(device, encoder, texture, width, height, format).map(
            |(buffer, padded_row_bytes)| FrameCaptureReadback {
                buffer,
                padded_row_bytes,
                width,
                height,
                format,
            },
        );
        Self { request, readback }
    }

    /// Start the readback once the frame carrying the copy has been submitted.
    ///
    /// The mapping callback only runs while the device is polled, and this compositor renders
    /// on demand, so a bounded worker owns that wait instead of the UI thread. It is not a
    /// render thread and does not exist per frame: it is started by a request, waits only for
    /// the submission that carries the copy, delivers the frame, and exits.
    fn finish(self, device: &wgpu::Device, submission_index: wgpu::SubmissionIndex) {
        let Self { request, readback } = self;
        let readback = match readback {
            Ok(readback) => readback,
            Err(error) => {
                let _ = request.completion.send(Err(error));
                return;
            }
        };

        let device = device.clone();
        let completion = request.completion;
        let spawned = std::thread::Builder::new()
            .name("frame-capture-readback".to_string())
            .spawn(move || {
                let _ = completion.send(readback.read(&device, submission_index));
            });
        if spawned.is_err() {
            // The worker never ran, so its completion sender was dropped and the caller
            // observes a cancelled channel rather than a capture that silently never lands.
            eprintln!("wgpui: could not start the frame capture readback worker");
        }
    }
}

/// A submitted copy of the complete-scene texture, waiting to be mapped.
struct FrameCaptureReadback {
    buffer: wgpu::Buffer,
    /// Row stride of the copy, padded to `wgpu`'s copy alignment.
    padded_row_bytes: u32,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl FrameCaptureReadback {
    /// Wait for the copy's submission, map it, and pack the frame.
    fn read(
        self,
        device: &wgpu::Device,
        submission_index: wgpu::SubmissionIndex,
    ) -> Result<FrameCapture, FrameCaptureError> {
        let slice = self.buffer.slice(..);
        let (mapped_sender, mapped_receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = mapped_sender.send(result);
        });

        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission_index),
                timeout: Some(CAPTURE_POLL_TIMEOUT),
            })
            .map_err(FrameCaptureError::DevicePoll)?;
        mapped_receiver
            .recv_timeout(CAPTURE_POLL_TIMEOUT)
            .map_err(|_| FrameCaptureError::BufferMap)?
            .map_err(|_| FrameCaptureError::BufferMap)?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|_| FrameCaptureError::BufferMap)?;
        let bytes = pack_capture_rows(&mapped, self.padded_row_bytes, self.width, self.height);
        drop(mapped);
        self.buffer.unmap();
        Ok(FrameCapture {
            width: self.width,
            height: self.height,
            format: self.format,
            bytes,
        })
    }
}

/// Pixel size of the surface formats this readback can copy.
fn capture_bytes_per_pixel(format: wgpu::TextureFormat) -> Option<usize> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => Some(4),
        _ => None,
    }
}

/// Copy all of `texture` into a mappable buffer, returning the buffer and its padded stride.
///
/// The copy is encoded into the frame's own encoder, so the bytes come from the very texture
/// that frame is about to present: the capture is the presented frame, not a re-render.
fn encode_capture_copy(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> Result<(wgpu::Buffer, u32), FrameCaptureError> {
    if width == 0 || height == 0 {
        return Err(FrameCaptureError::InvalidDimensions);
    }
    let bytes_per_pixel =
        capture_bytes_per_pixel(format).ok_or(FrameCaptureError::UnsupportedFormat(format))? as u32;
    let row_bytes = width * bytes_per_pixel;
    let padded_row_bytes =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame_capture"),
        size: u64::from(padded_row_bytes) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok((buffer, padded_row_bytes))
}

/// Pack mapped rows into a tightly packed buffer, dropping the copy alignment padding.
///
/// Every format this readback accepts is four bytes per pixel, so a tight row is
/// `width * 4` bytes.
fn pack_capture_rows(mapped: &[u8], padded_row_bytes: u32, width: u32, height: u32) -> Vec<u8> {
    let row_bytes = width as usize * 4;
    let mut bytes = Vec::with_capacity(row_bytes * height as usize);
    for row in 0..height as usize {
        let start = row * padded_row_bytes as usize;
        bytes.extend_from_slice(&mapped[start..start + row_bytes]);
    }
    bytes
}

impl WgpuRenderer {
    pub fn new<WindowHandle>(
        context: Arc<WgpuContext>,
        window: WindowHandle,
        atlas: Arc<WgpuAtlas>,
        width: u32,
        height: u32,
        path_sample_count: u32,
    ) -> anyhow::Result<Self>
    where
        WindowHandle: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    {
        let surface = unsafe {
            context
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(window.display_handle()?.as_raw()),
                    raw_window_handle: window.window_handle()?.as_raw(),
                })?
        };

        let surface_capabilities = surface.get_capabilities(&context.adapter);

        // NOTE(mdeand): The shaders (hsla_to_rgba) output sRGB values directly, so we need a
        // NOTE(mdeand): non-sRGB surface format to avoid a double linear-to-sRGB conversion.
        // NOTE(mdeand): Prefer a non-sRGB format; fall back to whatever is available.
        let format = surface_capabilities
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_capabilities.formats[0]);

        let alpha_mode = if surface_capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            surface_capabilities.alpha_modes[0]
        };

        // allow overriding vsync behaviour.  The default is `Fifo` (vsync
        // enabled) which is what `wgpu` considers the safest presentation mode.
        // Setting `WGPUI_DISABLE_VSYNC=1` in the environment will switch to
        // `Immediate`, which drops frames at the display's full rate.  A more
        // fine‑grained control (`WGPUI_PRESENT_MODE=mailbox|fifo|immediate`) is
        // also supported for experimentation.
        let present_mode = std::env::var("WGPUI_PRESENT_MODE")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "mailbox" => Some(wgpu::PresentMode::Mailbox),
                "immediate" => Some(wgpu::PresentMode::Immediate),
                "fifo" => Some(wgpu::PresentMode::Fifo),
                _ => None,
            })
            .unwrap_or_else(|| {
                if std::env::var("WGPUI_DISABLE_VSYNC").is_ok() {
                    wgpu::PresentMode::Immediate
                } else {
                    wgpu::PresentMode::Fifo
                }
            });

        let surface_configuration = wgpu::SurfaceConfiguration {
            // COPY_SRC is what lets a requested capture read back the complete-scene frame this
            // surface holds. It is part of the surface's permanent configuration rather than a
            // per-capture toggle, and its one cost — `CAMetalLayer.framebufferOnly` becomes
            // false on Metal — is paid whether or not a capture is ever requested. No copy is
            // encoded unless a capture was requested.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            // TODO(mdeand): Make this configurable?
            desired_maximum_frame_latency: 2,
        };

        Self::with_target(
            context,
            Some(surface),
            surface_configuration,
            atlas,
            path_sample_count,
        )
    }

    fn with_target(
        context: Arc<WgpuContext>,
        surface: Option<wgpu::Surface<'static>>,
        surface_configuration: wgpu::SurfaceConfiguration,
        atlas: Arc<WgpuAtlas>,
        path_sample_count: u32,
    ) -> anyhow::Result<Self> {
        let atlas_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let surface_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("surface_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pipelines =
            WgpuPipelines::new(context.as_ref(), &surface_configuration, path_sample_count);

        let mut renderer = Self {
            context,
            surface,
            offscreen_texture: None,
            surface_configuration,
            atlas,
            atlas_sampler,
            surface_sampler,
            pipelines,
            rendering_parameters: RenderingParameters::from_env(),
            path_intermediate_texture: None,
            path_intermediate_view: None,
            surface_bind_groups: Mutex::new(HashMap::new()),
            configure_pending: std::sync::atomic::AtomicBool::new(false),
            frame_capture_requests: FrameCaptureRequests::default(),
        };
        renderer.ensure_offscreen_texture();
        // Configure the surface for presentation before the first draw call.
        renderer.configure_surface();
        renderer.ensure_path_intermediate();
        Ok(renderer)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn new_headless() -> anyhow::Result<Self> {
        let context = Arc::new(WgpuContext::new()?);
        let atlas = Arc::new(WgpuAtlas::new(context.clone()));
        let configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Rgba8Unorm,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        Self::with_target(context, None, configuration, atlas, 4)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn read_offscreen_image(&self) -> anyhow::Result<image::RgbaImage> {
        let texture = self
            .offscreen_texture
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("renderer has no offscreen target"))?;
        let width = self.surface_configuration.width;
        let height = self.surface_configuration.height;
        let row_bytes = width
            .checked_mul(4)
            .ok_or_else(|| anyhow::anyhow!("image row size overflow"))?;
        let padded_row_bytes = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("WGPUI offscreen image readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("WGPUI screenshot copy"),
                });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.context.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if sender.send(result).is_err() {
                    log::error!("offscreen image readback receiver was dropped");
                }
            });
        self.context.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        receiver.recv_timeout(std::time::Duration::from_secs(30))??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let mut pixels = Vec::with_capacity((u64::from(row_bytes) * u64::from(height)) as usize);
        for row in mapped.chunks_exact(padded_row_bytes as usize) {
            pixels.extend_from_slice(
                row.get(..row_bytes as usize)
                    .ok_or_else(|| anyhow::anyhow!("GPU readback row is incomplete"))?,
            );
        }
        drop(mapped);
        buffer.unmap();
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("GPU readback image dimensions are invalid"))
    }

    /// Configure the surface, keeping a failure for a retry rather than
    /// panicking on it. Returns whether it is configured.
    fn configure_surface(&self) -> bool {
        let scope = self
            .context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let Some(surface) = &self.surface else {
            return true;
        };
        surface.configure(&self.context.device, &self.surface_configuration);
        let error = pollster::block_on(scope.pop());
        if let Some(error) = &error {
            log::warn!("surface configure failed, retrying next frame: {error}");
        }
        self.configure_pending
            .store(error.is_some(), std::sync::atomic::Ordering::Relaxed);
        error.is_none()
    }

    pub fn draw(&self, scene: &Scene) {
        if self
            .configure_pending
            .load(std::sync::atomic::Ordering::Relaxed)
            && !self.configure_surface()
        {
            return;
        }
        let mut command_encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("main"),
                });

        self.atlas.before_frame(&mut command_encoder);

        // Keep track of the exact texture generations rendered this frame.
        let mut seen_surface_generations: Vec<(crate::platform::surface_registry::SurfaceId, u64, usize)> =
            Vec::new();

        let color_adjustments = ColorAdjustments {
            gamma_ratios: self.rendering_parameters.gamma_ratios,
            grayscale_enhanced_contrast: self.rendering_parameters.grayscale_enhanced_contrast,
            _padding: [0.0; 3],
        };
        self.context.queue.write_buffer(
            &self.context.color_adjustments_buffer,
            0,
            bytemuck::bytes_of(&color_adjustments),
        );

        let globals = GlobalParams {
            viewport_size: [
                self.surface_configuration.width as f32,
                self.surface_configuration.height as f32,
            ],
            premultimated_alpha: match self.surface_configuration.alpha_mode {
                wgpu::CompositeAlphaMode::PreMultiplied => 1,
                _ => 0,
            },
            pad: 0,
        };

        self.context.queue.write_buffer(
            &self.context.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );

        unsafe fn as_bytes<T>(slice: &[T]) -> &[u8] {
            unsafe {
                std::slice::from_raw_parts(
                    slice.as_ptr() as *const u8,
                    std::mem::size_of_val(slice),
                )
            }
        }

        if !scene.quads.is_empty() {
            self.context
                .queue
                .write_buffer(&self.context.quads_buffer, 0, unsafe {
                    as_bytes(&scene.quads)
                });
        }
        if !scene.shadows.is_empty() {
            self.context
                .queue
                .write_buffer(&self.context.shadows_buffer, 0, unsafe {
                    as_bytes(&scene.shadows)
                });
        }
        if !scene.underlines.is_empty() {
            self.context
                .queue
                .write_buffer(&self.context.underlines_buffer, 0, unsafe {
                    as_bytes(&scene.underlines)
                });
        }
        if !scene.monochrome_sprites.is_empty() {
            self.context
                .queue
                .write_buffer(&self.context.mono_sprites_buffer, 0, unsafe {
                    as_bytes(&scene.monochrome_sprites)
                });
        }
        if !scene.polychrome_sprites.is_empty() {
            self.context
                .queue
                .write_buffer(&self.context.poly_sprites_buffer, 0, unsafe {
                    as_bytes(&scene.polychrome_sprites)
                });
        }

        let surface_texture = if let Some(surface) = &self.surface {
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture)
                | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Some(texture),
                _ => return,
            }
        } else {
            None
        };

        let quads_bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("quads_bind_group"),
                layout: &self.pipelines.quads_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.context.quads_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        let shadows_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("shadows_bind_group"),
                    layout: &self.pipelines.shadows_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.shadows_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        let underlines_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("underlines_bind_group"),
                    layout: &self.pipelines.underlines_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.underlines_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        let mono_sprites_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mono_sprites_bind_group"),
                    layout: &self.pipelines.mono_sprites_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.mono_sprites_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        let poly_sprites_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("poly_sprites_bind_group"),
                    layout: &self.pipelines.poly_sprites_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.poly_sprites_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        let frame_view = if let Some(surface_texture) = &surface_texture {
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        } else if let Some(texture) = &self.offscreen_texture {
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            return;
        };

        let mut pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &frame_view,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                resolve_target: None,
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        let mut quads_first_instance: u32 = 0;
        let mut shadows_first_instance: u32 = 0;
        let mut underlines_first_instance: u32 = 0;
        let mut mono_sprites_first_instance: u32 = 0;
        let mut poly_sprites_first_instance: u32 = 0;
        // Paths are the one primitive that needs two shared buffers, so each batch
        // gets its own region of both. Every write in a frame is enqueued before the
        // frame's single submit, and a later write to the same range wins for every
        // draw in that submit, so writing each batch at offset zero would leave every
        // batch but the last rasterizing and compositing the last batch's geometry.
        let mut paths_first_vertex: u32 = 0;
        let mut paths_first_sprite: u32 = 0;

        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Quads(quads) => {
                    let count = quads.len() as u32;
                    pass.set_pipeline(&self.pipelines.quads_pipeline);
                    pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                    pass.set_bind_group(1, &quads_bind_group, &[]);
                    pass.draw(0..4, quads_first_instance..quads_first_instance + count);
                    quads_first_instance += count;
                }

                PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => {
                    let count = sprites.len() as u32;
                    let tex_info = self.atlas.get_texture_info(texture_id);

                    let sprites_texture_bind_group =
                        self.context
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("sprites_bind_group"),
                                layout: &self.pipelines.sprites_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(
                                            &tex_info.raw_view,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.atlas_sampler,
                                        ),
                                    },
                                ],
                            });

                    pass.set_pipeline(&self.pipelines.mono_sprites_pipeline);
                    pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                    pass.set_bind_group(1, &self.pipelines.color_adjustments_bind_group, &[]);
                    pass.set_bind_group(2, &sprites_texture_bind_group, &[]);
                    pass.set_bind_group(3, &mono_sprites_bind_group, &[]);
                    pass.draw(
                        0..4,
                        mono_sprites_first_instance..mono_sprites_first_instance + count,
                    );
                    mono_sprites_first_instance += count;
                }
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => {
                    let count = sprites.len() as u32;
                    let tex_info = self.atlas.get_texture_info(texture_id);

                    let sprites_texture_bind_group =
                        self.context
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("poly_sprites_texture_bind_group"),
                                layout: &self.pipelines.sprites_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(
                                            &tex_info.raw_view,
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &self.atlas_sampler,
                                        ),
                                    },
                                ],
                            });

                    pass.set_pipeline(&self.pipelines.poly_sprites_pipeline);
                    pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                    pass.set_bind_group(1, &sprites_texture_bind_group, &[]);
                    pass.set_bind_group(2, &poly_sprites_bind_group, &[]);
                    pass.draw(
                        0..4,
                        poly_sprites_first_instance..poly_sprites_first_instance + count,
                    );
                    poly_sprites_first_instance += count;
                }
                PrimitiveBatch::Shadows(shadows) => {
                    let count = shadows.len() as u32;
                    pass.set_pipeline(&self.pipelines.shadows_pipeline);
                    pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                    pass.set_bind_group(1, &shadows_bind_group, &[]);
                    pass.draw(0..4, shadows_first_instance..shadows_first_instance + count);
                    shadows_first_instance += count;
                }
                PrimitiveBatch::Underlines(underlines) => {
                    let count = underlines.len() as u32;
                    pass.set_pipeline(&self.pipelines.underlines_pipeline);
                    pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                    pass.set_bind_group(1, &underlines_bind_group, &[]);
                    pass.draw(
                        0..4,
                        underlines_first_instance..underlines_first_instance + count,
                    );
                    underlines_first_instance += count;
                }
                PrimitiveBatch::Surfaces(surfaces) => {
                    for surface in surfaces {
                        let crate::SurfaceContent::Wgpu(surface_id) = &surface.content;
                        if let Some((idx, revision, views)) =
                            self.context.surface_registry.binding_snapshot(*surface_id)
                        {
                            // consuming the front view means the frame has been
                            // queued for compositing, so clear the pending flag
                            self.context
                                .surface_registry
                                .clear_present_pending(*surface_id);

                            let params = SurfaceParams {
                                bounds: Bounds {
                                    origin: [surface.bounds.origin.x.0, surface.bounds.origin.y.0],
                                    size: [
                                        surface.bounds.size.width.0,
                                        surface.bounds.size.height.0,
                                    ],
                                },
                                content_mask: Bounds {
                                    origin: [
                                        surface.content_mask.bounds.origin.x.0,
                                        surface.content_mask.bounds.origin.y.0,
                                    ],
                                    size: [
                                        surface.content_mask.bounds.size.width.0,
                                        surface.content_mask.bounds.size.height.0,
                                    ],
                                },
                            };

                            // fetch or create this surface's cached params
                            // buffer and bind groups
                            let surface_bind_group = {
                                let mut cache = self.surface_bind_groups.lock();
                                let entry =
                                    cache.entry((*surface_id, revision, seen_surface_generations.len())).or_insert_with(|| {
                                        let params_buffer = self.context.device.create_buffer(
                                            &wgpu::BufferDescriptor {
                                                label: Some("Surface Params Buffer"),
                                                size: std::mem::size_of::<SurfaceParams>() as u64,
                                                usage: wgpu::BufferUsages::UNIFORM
                                                    | wgpu::BufferUsages::COPY_DST,
                                                mapped_at_creation: false,
                                            },
                                        );
                                        // create both groups for front index 0 and 1
                                        let create_bg = |view: &wgpu::TextureView| {
                                            self.context.device.create_bind_group(
                                                &wgpu::BindGroupDescriptor {
                                                    label: Some("surface_bind_group"),
                                                    layout: &self
                                                        .pipelines
                                                        .surfaces_bind_group_layout,
                                                    entries: &[
                                                        wgpu::BindGroupEntry {
                                                            binding: 0,
                                                            resource: wgpu::BindingResource::Buffer(
                                                                wgpu::BufferBinding {
                                                                    buffer: &params_buffer,
                                                                    offset: 0,
                                                                    size: None,
                                                                },
                                                            ),
                                                        },
                                                        wgpu::BindGroupEntry {
                                                            binding: 1,
                                                            resource:
                                                                wgpu::BindingResource::TextureView(
                                                                    view,
                                                                ),
                                                        },
                                                        wgpu::BindGroupEntry {
                                                            binding: 2,
                                                            resource:
                                                                wgpu::BindingResource::Sampler(
                                                                    &self.surface_sampler,
                                                                ),
                                                        },
                                                    ],
                                                },
                                            )
                                        };
                                        let bind_groups =
                                            [create_bg(&views[0]), create_bg(&views[1])];
                                        SurfaceBinding {
                                            params: params_buffer,
                                            bind_groups,
                                        }
                                    });
                                self.context.queue.write_buffer(
                                    &entry.params,
                                    0,
                                    bytemuck::bytes_of(&params),
                                );
                                entry.bind_groups[idx].clone()
                            };

                            pass.set_pipeline(&self.pipelines.surfaces_pipeline);
                            pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
                            pass.set_bind_group(1, &surface_bind_group, &[]);
                            pass.draw(0..4, 0..1);

                            seen_surface_generations.push((*surface_id, revision, seen_surface_generations.len()));
                        }
                    }
                }
                PrimitiveBatch::Paths(paths) => {
                    drop(pass);
                    let rasterized =
                        self.rasterize_paths(&mut command_encoder, paths, paths_first_vertex);
                    paths_first_vertex += rasterized;
                    pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("main_continued"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame_view,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            resolve_target: None,
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    if rasterized > 0 {
                        paths_first_sprite +=
                            self.composite_paths(&mut pass, paths, paths_first_sprite);
                    }
                }
            }
        }

        drop(pass);

        // remove cached bind groups for surfaces that disappeared this frame
        {
            let mut cache = self.surface_bind_groups.lock();
            cache.retain(|key, _| seen_surface_generations.contains(key));
        }

        // A requested capture reads the very texture this frame is about to present, through
        // this frame's own encoder. Taking the request here is what makes it one-shot: it is
        // served by the frame that takes it, and no frame copies anything without a request.
        let capture = self.frame_capture_requests.take().map(|request| {
            let configuration = &self.surface_configuration;
            PendingFrameCapture::encode(
                request,
                &self.context.device,
                &mut command_encoder,
                surface_texture
                    .as_ref()
                    .map(|frame| &frame.texture)
                    .or(self.offscreen_texture.as_ref())
                    .expect("a drawable target exists"),
                configuration.width,
                configuration.height,
                configuration.format,
            )
        });

        let submission_index = self.context.queue.submit(Some(command_encoder.finish()));
        if let Some(surface_texture) = surface_texture {
            self.context.queue.present(surface_texture);
        }

        if let Some(capture) = capture {
            capture.finish(&self.context.device, submission_index);
        }
    }

    /// Install one pending capture of the next complete-scene frame.
    ///
    /// The receiver resolves once that frame has been presented and its copy mapped. A request
    /// that arrives before the previous one was served supersedes it: the older receiver is
    /// cancelled rather than answered with a frame captured later than it asked for.
    pub(crate) fn request_frame_capture(
        &self,
    ) -> oneshot::Receiver<Result<FrameCapture, FrameCaptureError>> {
        let (completion, receiver) = oneshot::channel();
        // The superseded request is dropped here, which cancels its receiver — the caller
        // learns the capture will not arrive instead of waiting on an unserved request.
        drop(
            self.frame_capture_requests
                .install(FrameCaptureRequest { completion }),
        );
        receiver
    }

    pub fn update_drawable_size(&mut self, size: geometry::Size<DevicePixels>) {
        self.surface_configuration.width = size.width.0 as u32;
        self.surface_configuration.height = size.height.0 as u32;
        self.configure_surface();
        self.ensure_offscreen_texture();
        self.ensure_path_intermediate();
    }

    fn ensure_offscreen_texture(&mut self) {
        if self.surface.is_some() {
            return;
        }
        self.offscreen_texture = Some(self.context.device.create_texture(
            &wgpu::TextureDescriptor {
                label: Some("WGPUI offscreen color target"),
                size: wgpu::Extent3d {
                    width: self.surface_configuration.width.max(1),
                    height: self.surface_configuration.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_configuration.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        ));
    }

    fn ensure_path_intermediate(&mut self) {
        let width = self.surface_configuration.width.max(1);
        let height = self.surface_configuration.height.max(1);
        let texture = self
            .context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("path_intermediate"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        self.path_intermediate_view =
            Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.path_intermediate_texture = Some(texture);
    }

    /// Rasterize one `PrimitiveBatch::Paths` into the intermediate texture at
    /// `first_vertex`, and return how many vertices were written — the caller advances
    /// its region by that count, so a batch that draws nothing (or does not fit) does
    /// not consume any of the shared buffer.
    fn rasterize_paths(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        paths: &[crate::Path<crate::ScaledPixels>],
        first_vertex: u32,
    ) -> u32 {
        let mut vertices = Vec::new();
        for path in paths {
            let clipped = path.clipped_bounds();
            let bounds = [
                clipped.origin.x.0,
                clipped.origin.y.0,
                clipped.size.width.0,
                clipped.size.height.0,
            ];
            vertices.extend(path.vertices.iter().map(|vertex| PathRasterizationVertex {
                xy_position: [vertex.xy_position.x.0, vertex.xy_position.y.0],
                st_position: [vertex.st_position.x, vertex.st_position.y],
                color: path.color,
                bounds,
            }));
        }
        if vertices.is_empty() {
            return 0;
        }

        let Some(path_view) = self.path_intermediate_view.as_ref() else {
            return 0;
        };

        unsafe fn as_bytes<T>(slice: &[T]) -> &[u8] {
            unsafe {
                std::slice::from_raw_parts(
                    slice.as_ptr() as *const u8,
                    std::mem::size_of_val(slice),
                )
            }
        }

        let vertex_bytes = std::mem::size_of::<PathRasterizationVertex>();
        let offset = first_vertex as u64 * vertex_bytes as u64;
        let bytes = unsafe { as_bytes(&vertices) };
        if offset + bytes.len() as u64 > self.context.path_vertices_buffer.size() {
            // The frame has more path geometry than the shared vertex buffer holds.
            // Draw nothing rather than writing past the end of the buffer.
            return 0;
        }
        self.context
            .queue
            .write_buffer(&self.context.path_vertices_buffer, offset, bytes);

        let vertices_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("path_rasterization_bind_group"),
                    layout: &self.pipelines.path_rasterization_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.path_vertices_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("path_rasterization_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: path_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.path_rasterization_pipeline);
            pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
            pass.set_bind_group(1, &vertices_bind_group, &[]);
            pass.draw(first_vertex..first_vertex + vertices.len() as u32, 0..1);
        }

        vertices.len() as u32
    }

    /// Composite one `PrimitiveBatch::Paths` from the intermediate texture, at
    /// `first_sprite`, and return how many sprites were drawn — the caller advances its
    /// region by that count.
    fn composite_paths(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        paths: &[crate::Path<crate::ScaledPixels>],
        first_sprite: u32,
    ) -> u32 {
        let Some(path_view) = self.path_intermediate_view.as_ref() else {
            return 0;
        };
        if paths.is_empty() {
            return 0;
        }

        let first = &paths[0];
        let sprites: Vec<PathSprite> = if paths.last().map(|path| &path.order) == Some(&first.order)
        {
            paths
                .iter()
                .map(|path| {
                    let clipped = path.clipped_bounds();
                    PathSprite {
                        bounds: [
                            clipped.origin.x.0,
                            clipped.origin.y.0,
                            clipped.size.width.0,
                            clipped.size.height.0,
                        ],
                    }
                })
                .collect()
        } else {
            let mut clipped = first.clipped_bounds();
            for path in paths.iter().skip(1) {
                clipped = clipped.union(&path.clipped_bounds());
            }
            vec![PathSprite {
                bounds: [
                    clipped.origin.x.0,
                    clipped.origin.y.0,
                    clipped.size.width.0,
                    clipped.size.height.0,
                ],
            }]
        };

        unsafe fn as_bytes<T>(slice: &[T]) -> &[u8] {
            unsafe {
                std::slice::from_raw_parts(
                    slice.as_ptr() as *const u8,
                    std::mem::size_of_val(slice),
                )
            }
        }

        let sprite_bytes = std::mem::size_of::<PathSprite>();
        let offset = first_sprite as u64 * sprite_bytes as u64;
        let bytes = unsafe { as_bytes(&sprites) };
        if offset + bytes.len() as u64 > self.context.path_sprites_buffer.size() {
            // The frame has more path sprites than the shared sprite buffer holds.
            // Composite nothing rather than writing past the end of the buffer.
            return 0;
        }
        self.context
            .queue
            .write_buffer(&self.context.path_sprites_buffer, offset, bytes);

        let sprites_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("path_sprites_bind_group"),
                    layout: &self.pipelines.path_sprites_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.context.path_sprites_buffer,
                            offset: 0,
                            size: None,
                        }),
                    }],
                });

        let texture_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("path_intermediate_texture_bind_group"),
                    layout: &self.pipelines.sprites_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(path_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                        },
                    ],
                });

        pass.set_pipeline(&self.pipelines.paths_pipeline);
        pass.set_bind_group(0, &self.pipelines.globals_bind_group, &[]);
        pass.set_bind_group(1, &sprites_bind_group, &[]);
        pass.set_bind_group(2, &texture_bind_group, &[]);
        pass.draw(0..4, first_sprite..first_sprite + sprites.len() as u32);

        sprites.len() as u32
    }

    #[allow(dead_code)]
    pub fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    #[allow(dead_code)]
    pub fn gpu_specs(&self) -> GpuSpecs {
        let info = self.context.adapter.get_info();
        GpuSpecs {
            is_software_emulated: info.device_type == wgpu::DeviceType::Cpu,
            device_name: info.name,
            driver_name: info.driver,
            driver_info: info.driver_info,
        }
    }

    #[allow(dead_code)]
    pub fn update_transparency(&mut self, transparent: bool) {
        self.surface_configuration.alpha_mode = if transparent {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            // Opaque vs premultiplied is compositor-dependent; Inherit lets
            // wgpu pick a mode the surface actually supports.
            wgpu::CompositeAlphaMode::Inherit
        };
        self.configure_surface();
        self.ensure_offscreen_texture();
    }

    #[allow(dead_code)]
    pub fn viewport_size(&self) -> geometry::Size<DevicePixels> {
        geometry::Size {
            width: DevicePixels(self.surface_configuration.width as i32),
            height: DevicePixels(self.surface_configuration.height as i32),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl crate::PlatformHeadlessRenderer for WgpuRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: crate::Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.render_scene(scene, size)?;
        self.read_offscreen_image()
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        size: crate::Size<DevicePixels>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "offscreen target dimensions must be positive"
        );
        let maximum = self.context.device.limits().max_texture_dimension_2d;
        anyhow::ensure!(
            size.width.0 as u32 <= maximum && size.height.0 as u32 <= maximum,
            "offscreen target exceeds the GPU texture size limit"
        );
        if self.viewport_size() != size {
            self.update_drawable_size(size);
        }
        self.draw(scene);
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

#[cfg(test)]
mod offscreen_tests {
    use super::*;
    use crate::{Bounds, ContentMask, PlatformHeadlessRenderer, point, rgb, size};

    #[test]
    #[ignore = "requires a usable WGPU adapter"]
    fn offscreen_readback_preserves_pixels_padding_and_resize() -> anyhow::Result<()> {
        let mut renderer = WgpuRenderer::new_headless()?;
        for (width, height, color, expected) in [
            (13, 7, 0xff0000, [255, 0, 0, 255]),
            (67, 3, 0x00ff00, [0, 255, 0, 255]),
            (5, 9, 0x0000ff, [0, 0, 255, 255]),
        ] {
            let mut scene = Scene::default();
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(width as f32), ScaledPixels(height as f32)),
            );
            scene.insert_primitive(Quad {
                bounds,
                content_mask: ContentMask { bounds },
                background: rgb(color).into(),
                ..Default::default()
            });
            scene.finish();
            let image = renderer
                .render_scene_to_image(&scene, size(DevicePixels(width), DevicePixels(height)))?;
            assert_eq!(image.dimensions(), (width as u32, height as u32));
            assert!(
                image.pixels().all(|pixel| pixel.0 == expected),
                "GPU pixels differ after padded readback or resize"
            );
            let repeated = renderer
                .render_scene_to_image(&scene, size(DevicePixels(width), DevicePixels(height)))?;
            assert_eq!(
                image, repeated,
                "unchanged scenes must produce deterministic GPU pixels"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_request_is_served_by_exactly_one_frame() {
        let requests = FrameCaptureRequests::default();
        let (completion, _receiver) = oneshot::channel();
        assert!(
            requests
                .install(FrameCaptureRequest { completion })
                .is_none()
        );

        let taken = requests.take().expect("the installed request is pending");
        drop(taken);

        assert!(
            requests.take().is_none(),
            "the frame that took the request is the only frame that serves it"
        );
    }

    #[test]
    fn a_second_request_supersedes_the_first() {
        let requests = FrameCaptureRequests::default();
        let (first_completion, mut first_receiver) = oneshot::channel();
        requests.install(FrameCaptureRequest {
            completion: first_completion,
        });

        let (second_completion, _second_receiver) = oneshot::channel();
        let superseded = requests
            .install(FrameCaptureRequest {
                completion: second_completion,
            })
            .expect("the second request supersedes the first");
        drop(superseded);

        assert!(matches!(first_receiver.try_recv(), Err(oneshot::Canceled)));
        assert!(
            requests.take().is_some(),
            "the newest request is the one still pending"
        );
    }

    #[test]
    fn captured_rows_drop_the_readback_padding() {
        // Two rows of four pixels each, padded to wgpu's copy alignment.
        let padded_row_bytes: u32 = 256;
        let mut mapped = vec![0u8; padded_row_bytes as usize * 2];
        mapped[..16].copy_from_slice(&[1u8; 16]);
        mapped[padded_row_bytes as usize..padded_row_bytes as usize + 16]
            .copy_from_slice(&[2u8; 16]);

        let bytes = pack_capture_rows(&mapped, padded_row_bytes, 4, 2);

        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[..16], &[1u8; 16]);
        assert_eq!(&bytes[16..], &[2u8; 16]);
    }
}
