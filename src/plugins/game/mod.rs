use crate::plugins::*;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use iyes_loopless::prelude::{AppLooplessStateExt, IntoConditionalSystem};
use leafwing_input_manager::prelude::ActionState;

use super::{
    asset_processor::{Level, LevelProcessor},
    first_person_controller::{FirstPersonCamera, FirstPersonController},
    input::Actions,
    physics::*,
    portal::PortalTeleport,
};

// region:    --- Asset Constants
const CROSSHAIR_SPRITE: &str = "crosshair.png";
// endregion: --- Game Constants

/// The different possible states of the game application.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum GameState {
    /// Player is in the main menu, or in a submenu.
    MainMenu,
    /// A level is currently loading.
    Loading,
    /// The player is in game.
    InGame,
    // The game is currently paused.
    //Paused
}

#[derive(Debug)]
/// Main game plugin, responsible for loading the other game plugins and bootstrapping the game.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);

        app.add_loopless_state(GameState::MainMenu);
        app.add_startup_system(game_startup);

        #[cfg(feature = "devel")]
        {
            app.add_plugins(debug::DeveloperPlugins);
        }

        app.add_plugin(RapierPhysicsPlugin::<NoUserData>::default());
        app.add_plugin(doors::DoorsPlugin);
        app.add_plugin(physics::PhysicsPlugin);
        app.add_plugin(portal::PortalPlugin);
        app.add_plugin(render::RenderPlugin);
        app.add_plugin(first_person_controller::FirstPersonControllerPlugin);
        app.add_plugin(input::InputPlugin);
        app.add_plugin(asset_processor::LevelsPlugin);

        app.add_startup_system_set(
            SystemSet::new()
                .with_system(game_startup)
                .with_system(init_resources),
        )
        //.add_startup_system_to_stage(StartupStage::PostStartup, crosshair)
        .add_system(load_level_when_ready.run_in_state(GameState::MainMenu))
        .add_system(throw_cube);
    }
}

#[derive(Debug, Clone, Default, Reflect)]
pub struct GameResources {
    cube_mesh: Handle<Mesh>,
    cube_material: Handle<StandardMaterial>,
    crosshair: Handle<Image>,
}

#[derive(Bundle)]
/// Defines the ECS components of a physically driven cube prop.
pub struct PhysicsCubeBundle {
    #[bundle]
    pbr_bundle: PbrBundle,
    collider: Collider,
    initial_velocity: Velocity,
    rigidbody: RigidBody,
    groups: CollisionGroups,
    teleport: PortalTeleport,
    ccd: Ccd,
}

impl Default for PhysicsCubeBundle {
    fn default() -> Self {
        PhysicsCubeBundle {
            pbr_bundle: PbrBundle::default(),
            collider: Collider::cuboid(CUBE_SIZE / 2., CUBE_SIZE / 2., CUBE_SIZE / 2.),
            initial_velocity: Velocity::default(),
            rigidbody: RigidBody::Dynamic,
            groups: CollisionGroups::new(PROPS_GROUP, ALL_GROUPS),
            teleport: PortalTeleport,
            ccd: Ccd::disabled(),
        }
    }
}

const CUBE_SIZE: f32 = 0.2;

fn init_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let mesh = meshes.add(shape::Cube { size: 0.2 }.into());
    let material = materials.add(StandardMaterial {
        base_color: Color::CYAN,
        perceptual_roughness: 0.,
        metallic: 0.,
        reflectance: 0.5,
        ..default()
    });
    commands.insert_resource(GameResources {
        cube_mesh: mesh,
        cube_material: material,
        crosshair: asset_server.load(CROSSHAIR_SPRITE),
    });
}

/// Throw a physically driven cube in front of the player.
fn throw_cube(
    mut commands: Commands,
    player_query: Query<&ActionState<Actions>, With<FirstPersonController>>,
    camera_query: Query<&GlobalTransform, With<FirstPersonCamera>>,
    res: Res<GameResources>,
) {
    if let (Ok(input), Ok(cam_trf)) = (player_query.get_single(), camera_query.get_single()) {
        if input.just_pressed(Actions::ShootCube) {
            let mut cube_trf = cam_trf.compute_transform();
            cube_trf.translation += cam_trf.forward();
            commands.spawn_bundle(PhysicsCubeBundle {
                pbr_bundle: PbrBundle {
                    mesh: res.cube_mesh.clone(),
                    material: res.cube_material.clone(),
                    transform: cube_trf,
                    ..default()
                },
                initial_velocity: Velocity {
                    linvel: cube_trf.forward() * 5.,
                    ..default()
                },
                ..default()
            });
        }
    }
}

pub const LOBBY_LEVEL_NAME: &str = "lobby";
pub const LOBBY_LEVEL_FILE: &str = "levels/level1.glb";

/// Perform game initialization
fn game_startup(assets: Res<AssetServer>, mut level_manager: ResMut<LevelProcessor>) {
    level_manager.load_level(LOBBY_LEVEL_FILE, LOBBY_LEVEL_NAME.to_owned(), &assets);
}

fn load_level_when_ready(
    mut commands: Commands,
    mut level_events: EventReader<AssetEvent<Level>>,
    mut level_manager: ResMut<LevelProcessor>,
    levels: Res<Assets<Level>>,
    mut loaded: Local<bool>,
) {
    for event in level_events.iter() {
        match event {
            AssetEvent::Created { handle } => {
                let level = levels.get(handle).unwrap();
                if level.name == LOBBY_LEVEL_NAME {
                    level_manager
                        .instantiate_level(&mut commands, LOBBY_LEVEL_NAME)
                        .expect("Can not instantiate level");
                    *loaded = true;
                }
            }
            AssetEvent::Modified { handle: _ } => {}
            AssetEvent::Removed { handle: _ } => {}
        }
    }
}
