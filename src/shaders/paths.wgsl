@group(1) @binding(0) var<storage, read> b_path_sprites: array<PathSprite>;
@group(2) @binding(0) var t_sprite: texture_2d<f32>;
@group(2) @binding(1) var s_sprite: sampler;

struct PathSprite {
    bounds: Bounds,
}

struct PathVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) texture_coords: vec2<f32>,
}

fn load_path_sprite(instance_id: u32) -> PathSprite {
    return b_path_sprites[instance_id];
}

@vertex
fn vs_path(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> PathVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
    let sprite = load_path_sprite(instance_id);
    let device_position = to_device_position(unit_vertex, sprite.bounds);
    let screen_position = sprite.bounds.origin + unit_vertex * sprite.bounds.size;
    let texture_coords = screen_position / globals.viewport_size;

    var out = PathVarying();
    out.position = device_position;
    out.texture_coords = texture_coords;
    return out;
}

@fragment
fn fs_path(input: PathVarying) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, input.texture_coords);
}
