#import bevy_pbr::mesh_view_bindings

struct GridParams {
    grid_strength: vec4<f32>,
    base_color: vec4<f32>
}

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;
@group(1) @binding(2)
var<uniform> params: GridParams;

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    #import bevy_pbr::mesh_vertex_output
) -> @location(0) vec4<f32> {
    let weights = normalize(abs(world_normal));
    let scale = 2.;
    let colorXY = textureSample(texture, texture_sampler, fract(world_position.xy / scale));
    let colorXZ = textureSample(texture, texture_sampler, fract(world_position.xz / scale));
    let colorZY = textureSample(texture, texture_sampler, fract(world_position.zy / scale));
    return (weights.x * colorZY + weights.y * colorXZ + weights.z * colorXY) * params.base_color;
    //return colorXY;
    //return vec4(fract(world_position.xyz), 1.);
    //return vec4(weights.xyz, 1.);
}
