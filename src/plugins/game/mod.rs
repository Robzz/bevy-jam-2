use std::f32::consts::*;

use crate::{plugins::*, util::scenes::make_test_arena};

use bevy::prelude::*;
use bevy_fps_controller::controller::*;
use bevy_rapier3d::prelude::*;

#[derive(Debug)]
/// Main game plugin, responsible for loading the other game plugins and bootstrapping the game.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);

        #[cfg(feature = "devel")]
        {
            app.add_plugins(dev_plugins::DeveloperPlugins);
        }

        app.add_plugin(RapierPhysicsPlugin::<NoUserData>::default());
        app.add_plugin(bevy_prototype_debug_lines::DebugLinesPlugin::default());
        app.add_plugin(FpsControllerPlugin);
        app.add_plugin(input::InputPlugin);
        app.add_plugin(portal::PortalPlugin);

        app.add_startup_system(setup);
    }
}

/// Perform game initialization
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    //asset_server: Res<AssetServer>
) {
    make_test_arena(&mut commands, &mut meshes, &mut materials, 20., 3.);

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

    // Spawn player
    commands
        .spawn()
        .insert(Collider::capsule(Vec3::Y * 0.5, Vec3::Y * 1.5, 0.5))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::zero())
        .insert(RigidBody::Dynamic)
        .insert(Sleeping::disabled())
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(AdditionalMassProperties::Mass(1.0))
        .insert(GravityScale(0.0))
        .insert(Ccd { enabled: true }) // Prevent clipping when going fast
        .insert(Transform::from_xyz(0.0, 3.0, 0.0))
        .insert(LogicalPlayer(0))
        .insert(FpsControllerInput { ..default() })
        .insert(FpsController {
            key_forward: KeyCode::Z,
            key_left: KeyCode::Q,
            key_back: KeyCode::S,
            key_right: KeyCode::D,
            ..default()
        });
    commands
        .spawn_bundle(Camera3dBundle::default())
        .insert(RenderPlayer(0));
}
