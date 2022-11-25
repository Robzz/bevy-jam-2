#import bevy_pbr::mesh_view_bindings

struct ClosedPortalMaterial {
    color: vec4<f32>,
}

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;
@group(1) @binding(2)
var<uniform> material: ClosedPortalMaterial;

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    #import bevy_pbr::mesh_vertex_output
) -> @location(0) vec4<f32> {
    let varying_uvs = vec2(fract(1. - pow((2. * uv.x) - 1., 2.) + globals.time), 1. - uv.y);
    let i = textureSample(texture, texture_sampler, varying_uvs).r;
    let color = vec4(i * material.color.rgb, 1.);
    return color;
}
