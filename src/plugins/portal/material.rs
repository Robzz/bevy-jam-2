use bevy::{prelude::*, render::render_resource::{AsBindGroup, ShaderRef}, reflect::TypeUuid};

#[derive(AsBindGroup, Debug, Clone, TypeUuid, Reflect)]
#[uuid = "04901b22-de12-43a9-8e2e-79d333201b93"]
pub struct PortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/portal.wgsl".into()
    }

    fn specialize(
        pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::render::mesh::MeshVertexBufferLayout,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
