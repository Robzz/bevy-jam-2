//! Rendering extras, like general purpose shaders.

use bevy::{
    prelude::*,
    reflect::{Reflect, TypeUuid},
    render::render_resource::{AsBindGroup, ShaderRef},
};

#[derive(Debug, Default, Reflect)]
pub struct RenderResources {
    pub grid_texture: Handle<Image>,
    pub default_grid_material: Handle<GridMaterial>,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GridMaterial>()
            .register_type::<RenderResources>()
            .add_plugin(MaterialPlugin::<GridMaterial>::default())
            .add_startup_system(load_render_textures);
    }
}

#[derive(AsBindGroup, Debug, Clone, TypeUuid, Reflect)]
#[uuid = "bac0548a-d97a-4d30-a275-18a4f0d1fc9f"]
/// Overlay a grid texture over non UV-unwrapped mesh, using the world coordinates as UVs.
/// Additional parameters allow changing the surface color and intensity of the grid texture.
pub struct GridMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    #[uniform(2)]
    pub grid_strength: Vec4,
    #[uniform(3)]
    pub base_color: Color,
}

impl Material for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wgsl".into()
    }
}

fn load_render_textures(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut grids: ResMut<Assets<GridMaterial>>,
) {
    let grid_texture = assets.load("textures/PolygonPrototype_Texture_Grid_01.png");
    let default_grid = grids.add(GridMaterial {
        texture: grid_texture.clone(),
        grid_strength: Vec4::ONE,
        base_color: Color::GRAY,
    });
    commands.insert_resource(RenderResources {
        grid_texture,
        default_grid_material: default_grid,
    });
}
