#import bevy_pbr::mesh_view_bindings

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;
@group(1) @binding(2)
var<uniform> grid_strength: vec4<f32>;
@group(1) @binding(3)
var<uniform> base_color: vec4<f32>;

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
    return (weights.x * colorZY + weights.y * colorXZ + weights.z * colorXY) * base_color;
    //return colorXY;
    //return vec4(fract(world_position.xyz), 1.);
    //return vec4(weights.xyz, 1.);
}
