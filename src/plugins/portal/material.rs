use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef, ShaderType},
};

#[derive(AsBindGroup, Debug, Clone, TypeUuid, Reflect)]
#[uuid = "04901b22-de12-43a9-8e2e-79d333201b93"]
pub struct OpenPortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
}

impl Material for OpenPortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/portal_open.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(AsBindGroup, Debug, Clone, TypeUuid, Reflect)]
#[uuid = "3373a227-c84e-4da6-bc1d-c7927ff75ef9"]
pub struct ClosedPortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    #[uniform(2)]
    pub uniform: ClosedPortalUniform,
}

impl Material for ClosedPortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/portal_closed.wgsl".into()
    }
}

#[derive(Debug, Clone, ShaderType, Reflect)]
pub struct ClosedPortalUniform {
    pub color: Color,
}
