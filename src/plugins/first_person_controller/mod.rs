//! This module contains the first person controller plugin.
//!
//! TODO features:
//!
//! * Additional controls:
//!   * Crouching
//! * Climbing slopes and stairs

use bevy::{prelude::*, reflect::FromReflect, render::camera::Projection};
use bevy_rapier3d::prelude::*;
use euclid::Angle;
use leafwing_input_manager::prelude::*;

use crate::plugins::{input::default_input_map, physics::*, portal::PortalTeleport};

use super::input::Actions;

#[derive(Debug)]
/// First person controller plugin, which registers the required systems to use the first person
/// controller also provided by this module.
pub struct FirstPersonControllerPlugin;

impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(spawn_controller.label(FirstPersonLabels::SpawnControllers))
            .add_system(process_controller_inputs.label(FirstPersonLabels::ProcessInputs));
    }
}

#[derive(Debug, SystemLabel)]
/// Labels for the first person controller systems.
pub enum FirstPersonLabels {
    SpawnControllers,
    ProcessInputs,
}

#[derive(Debug, Component)]
/// First person controller component.
pub struct FirstPersonController {
    pub yaw: Angle<f32>,
    pub pitch: Angle<f32>,
    pub camera_anchor: Entity,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
#[reflect(Component)]
/// Marker trait for first person cameras
pub struct FirstPersonCamera;

#[derive(Debug, Component, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct FirstPersonControllerSpawner {}

#[derive(Debug, Bundle, Default)]
pub struct FirstPersonControllerBundle {
    #[bundle]
    pub spatial: SpatialBundle,
    pub spawner: FirstPersonControllerSpawner,
}

#[derive(Debug, Component, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct CameraAnchor;

#[derive(Debug, Component, Clone, Default, Reflect, FromReflect)]
#[reflect(Component)]
/// Component that can be placed on the first player controller and/or camera to lock their
/// respective rotational degree of freedom.
pub struct CameraLock;

fn spawn_controller(
    mut commands: Commands,
    spawners_query: Query<(&FirstPersonControllerSpawner, Entity)>,
) {
    for (_spawner, id) in &spawners_query {
        const PLAYER_HEIGHT: f32 = 1.8;
        const EYE_HEIGHT: f32 = 0.6;
        const CAMERA_OFFSET: Vec3 = Vec3::new(0., EYE_HEIGHT - PLAYER_HEIGHT / 2., 0.);

        let player_root = commands
            .entity(id)
            .insert_bundle(InputManagerBundle {
                action_state: ActionState::default(),
                input_map: default_input_map(),
            })
            .insert_bundle((
                RigidBody::Dynamic,
                Ccd::enabled(),
                Collider::capsule_y(PLAYER_HEIGHT / 2., 0.4),
                ColliderMassProperties::MassProperties(MassProperties {
                    local_center_of_mass: Vec3::ZERO,
                    mass: 80.,
                    ..default()
                }),
                LockedAxes::ROTATION_LOCKED_X | LockedAxes::ROTATION_LOCKED_Z,
                Velocity::default(),
                Name::from("Player"),
                CollisionGroups::new(PLAYER_GROUP, ALL_GROUPS),
                PortalTeleport,
            ))
            .id();

        let camera_anchor = commands
            .spawn_bundle(SpatialBundle::from(Transform::from_translation(
                CAMERA_OFFSET,
            )))
            .insert_bundle((Name::from("Camera anchor"), CameraAnchor))
            .id();

        let camera = commands
            .spawn_bundle(Camera3dBundle {
                projection: Projection::Perspective(PerspectiveProjection {
                    fov: std::f32::consts::FRAC_PI_4,
                    // TODO: make the portal cameras use the main camera FOV so we can change this
                    aspect_ratio: 16. / 9.,
                    near: 0.1,
                    far: 1000.,
                }),
                ..default()
            })
            .insert_bundle((Name::from("Player camera"), FirstPersonCamera))
            .id();

        commands.entity(camera_anchor).push_children(&[camera]);

        commands
            .entity(player_root)
            .add_child(camera_anchor)
            .insert(FirstPersonController {
                yaw: Angle::zero(),
                pitch: Angle::zero(),
                camera_anchor,
            });

        commands.entity(id).remove::<FirstPersonControllerSpawner>();
    }
}

const PLAYER_SPEED: f32 = 3.;
const MOUSE_SENSITIVITY: f32 = 0.004;
const MOUSE_ANGVEL_MULTIPLIER: f32 = -75.;
const SPRINT_MULTIPLIER: f32 = 2.;

fn process_controller_inputs(
    mut player_query: Query<(
        &ActionState<Actions>,
        &mut FirstPersonController,
        &mut Velocity,
        &Transform,
        Option<&CameraLock>,
    )>,
    mut camera_query: Query<&mut Transform, (Without<FirstPersonController>, Without<CameraLock>)>,
) {
    for (input_state, mut controller, mut velocity, transform, yaw_lock) in &mut player_query {
        let mut new_velocities = Vec3::new(0., velocity.linvel.y, 0.);

        // Process movement on the forward axis
        let forward = transform.forward();
        match (
            input_state.pressed(Actions::Forward),
            input_state.pressed(Actions::Backwards),
            input_state.pressed(Actions::Sprint),
        ) {
            (true, false, sprint) => {
                let k = if sprint { SPRINT_MULTIPLIER } else { 1. };
                new_velocities.x = PLAYER_SPEED * k * forward.x;
                new_velocities.z = PLAYER_SPEED * k * forward.z;
            }
            (false, true, sprint) => {
                let k = if sprint { SPRINT_MULTIPLIER } else { 1. };
                new_velocities.x = -PLAYER_SPEED * k * forward.x;
                new_velocities.z = -PLAYER_SPEED * k * forward.z;
            }
            _ => {}
        }

        // Process movement on the lateral axis
        let left = transform.left();
        match (
            input_state.pressed(Actions::StrafeLeft),
            input_state.pressed(Actions::StrafeRight),
            input_state.pressed(Actions::Sprint),
        ) {
            (true, false, sprint) => {
                let k = if sprint { SPRINT_MULTIPLIER } else { 1. };
                new_velocities.x += PLAYER_SPEED * k * left.x;
                new_velocities.z += PLAYER_SPEED * k * left.z;
            }
            (false, true, sprint) => {
                let k = if sprint { SPRINT_MULTIPLIER } else { 1. };
                new_velocities.x += -PLAYER_SPEED * k * left.x;
                new_velocities.z += -PLAYER_SPEED * k * left.z;
            }
            _ => {}
        }

        const JUMP_SPEED: f32 = 6.0;
        if input_state.just_pressed(Actions::Jump) {
            new_velocities.y = JUMP_SPEED;
        }

        velocity.linvel = new_velocities;

        // Process mouse movement. We handle the rotation components separately:
        // * Rotation around the vertical axis (e.g. aiming left or right) is applied to the
        //   player root node.
        // * Rotation around the horizontal axis (e.g. aiming up or down) is applied directly to
        //   the perspective camera in order to keep the vertical orientation neutral on the root
        //   node.
        if let Some(mouse_movement) = input_state.axis_pair(Actions::Aim) {
            controller.yaw += Angle::radians(mouse_movement.x()) * MOUSE_SENSITIVITY;
            controller.pitch += Angle::radians(mouse_movement.y() * MOUSE_SENSITIVITY);
            controller.pitch.radians = controller
                .pitch
                .radians
                .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

            let v_rotation = Quat::from_axis_angle(Vec3::X, -controller.pitch.radians);
            if yaw_lock.is_none() {
                velocity.angvel.y =
                    mouse_movement.x() * MOUSE_SENSITIVITY * MOUSE_ANGVEL_MULTIPLIER;
            }

            if let Ok(mut camera_transform) = camera_query.get_mut(controller.camera_anchor) {
                camera_transform.rotation = v_rotation;
            }
        } else {
            velocity.angvel.y = 0.;
        }
    }
}
