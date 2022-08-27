use std::f32::consts::*;

use crate::{plugins::*, util::scenes::make_test_arena};

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::{
    first_person_controller::{
        FirstPersonCamera, FirstPersonController, FirstPersonControllerBundle,
    },
    input::Actions,
    physics::*,
    portal::PortalTeleport,
};

// region:    --- Asset Constants
const CROSSHAIR_SPRITE: &str = "crosshair.png";
// endregion: --- Game Constants

#[derive(Debug)]
/// Main game plugin, responsible for loading the other game plugins and bootstrapping the game.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);

        #[cfg(feature = "devel")]
        {
            app.add_plugins(debug::DeveloperPlugins);
        }

        app.add_plugin(RapierPhysicsPlugin::<NoUserData>::default());
        app.add_plugin(physics::PhysicsPlugin);
        app.add_plugin(portal::PortalPlugin);
        app.add_plugin(bevy_prototype_debug_lines::DebugLinesPlugin::default());
        app.add_plugin(first_person_controller::FirstPersonControllerPlugin);
        app.add_plugin(input::InputPlugin);

        app.add_startup_system_set(
            SystemSet::new()
                .with_system(setup)
                .with_system(init_resources),
        )
        .add_startup_system_to_stage(StartupStage::PostStartup, crosshair)
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
            ccd: Ccd::enabled(),
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

/// Perform game initialization
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    //asset_server: Res<AssetServer>
) {
    make_test_arena(&mut commands, &mut meshes, &mut materials, 20., 5.);

    // Light
    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::ANTIQUE_WHITE,
            illuminance: 20_000.,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::Y * 5.,
            rotation: Quat::from_euler(EulerRot::YXZ, FRAC_PI_4, FRAC_PI_4, 0.),
            scale: Vec3::ONE,
        },
        ..default()
    });

    // Player
    commands.spawn_bundle(FirstPersonControllerBundle {
        spatial: SpatialBundle {
            // The controller uses the center of mass as a reference
            transform: Transform::from_xyz(0., 1., 0.),
            ..default()
        },
        ..default()
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

fn crosshair(mut commands: Commands, res: Res<GameResources>) {
    // crosshair
    commands.spawn_bundle(SpriteBundle {
        texture: res.crosshair.clone(),
        transform: Transform {
            scale: Vec3::new(5., 5., 1.),
            ..Default::default()
        },
        ..Default::default()
    });
}
