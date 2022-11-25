//! This module contains the first person controller plugin.
//!
//! TODO features:
//!
//! * Additional controls:
//!   * Crouching
//! * Climbing slopes and stairs

use std::f32::consts::PI;

use bevy::{
    gltf::{Gltf, GltfMesh},
    prelude::*,
    reflect::FromReflect,
    render::camera::Projection,
};
use bevy_rapier3d::prelude::*;
use euclid::Angle;
use iyes_loopless::condition::IntoConditionalSystem;
use leafwing_input_manager::prelude::*;

use crate::plugins::{input::default_input_map, physics::*, portal::PortalTeleport};

use super::{
    asset_processor::{CurrentLevel, Level},
    game::{GameState, PlayerProgress},
    input::Actions,
};

#[derive(Debug)]
/// First person controller plugin, which registers the required systems to use the first person
/// controller also provided by this module.
pub struct FirstPersonControllerPlugin;

impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(
            spawn_controller
                .run_in_state(GameState::InGame)
                .label(FirstPersonLabels::SpawnControllers),
        )
        .add_system(
            process_controller_inputs
                .run_in_state(GameState::InGame)
                .label(FirstPersonLabels::ProcessInputs),
        )
        .add_system(
            show_gun_on_pickup
                .run_in_state(GameState::InGame)
                .label(FirstPersonLabels::ToggleGun),
        );
    }
}

#[derive(Debug, SystemLabel)]
/// Labels for the first person controller systems.
pub enum FirstPersonLabels {
    SpawnControllers,
    ProcessInputs,
    ToggleGun,
}

#[derive(Debug, Component)]
/// First person controller component.
pub struct FirstPersonController {
    pub yaw: Angle<f32>,
    pub pitch: Angle<f32>,
    pub camera_anchor: Entity,
    pub weapon_node: Entity,
    pub grabbed_object: Option<Entity>,
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

pub const PLAYER_HEIGHT: f32 = 1.8;
const EYE_HEIGHT: f32 = 1.5;
const CAMERA_OFFSET: Vec3 = Vec3::new(0., EYE_HEIGHT - PLAYER_HEIGHT / 2., 0.);

fn spawn_controller(
    mut commands: Commands,
    spawners_query: Query<(&FirstPersonControllerSpawner, Entity)>,
    current_level: Res<CurrentLevel>,
    levels: Res<Assets<Level>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    for (_spawner, id) in &spawners_query {
        let player_root = commands
            .entity(id)
            .insert(InputManagerBundle {
                action_state: ActionState::default(),
                input_map: default_input_map(),
            })
            .insert((
                RigidBody::Dynamic,
                Ccd::disabled(),
                Collider::capsule_y((PLAYER_HEIGHT - 0.8) / 2., 0.4),
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

        let level = levels.get(&current_level.get()).unwrap();
        let gltf = gltfs.get(&level.gltf).unwrap();
        let portal_gun_mesh = gltf_meshes
            .get(gltf.named_meshes.get("Scene.070").unwrap())
            .unwrap();
        let primitive = portal_gun_mesh.primitives.first().unwrap();
        let material = primitive.material.clone().unwrap();

        let gun_entity = commands
            .spawn(PbrBundle {
                mesh: primitive.mesh.clone(),
                material,
                transform: Transform {
                    translation: Vec3::new(0.2, -0.2, -0.6),
                    rotation: Quat::from_rotation_y(PI),
                    ..default()
                },
                visibility: Visibility { is_visible: false },
                ..default()
            })
            .id();

        let camera_anchor = commands
            .spawn(SpatialBundle::from(Transform::from_translation(
                CAMERA_OFFSET,
            )))
            .insert((Name::from("Camera anchor"), CameraAnchor))
            .id();

        let camera = commands
            .spawn(Camera3dBundle {
                projection: Projection::Perspective(PerspectiveProjection {
                    fov: std::f32::consts::FRAC_PI_4,
                    // TODO: make the portal cameras use the main camera FOV so we can change this
                    aspect_ratio: 16. / 9.,
                    near: 0.1,
                    far: 1000.,
                }),
                ..default()
            })
            .insert((Name::from("Player camera"), FirstPersonCamera))
            .id();

        commands
            .entity(camera_anchor)
            .push_children(&[camera, gun_entity]);

        commands
            .entity(player_root)
            .add_child(camera_anchor)
            .insert(FirstPersonController {
                yaw: Angle::zero(),
                pitch: Angle::zero(),
                camera_anchor,
                grabbed_object: None,
                weapon_node: gun_entity,
            });

        commands.entity(id).remove::<FirstPersonControllerSpawner>();
    }
}

const PLAYER_SPEED: f32 = 3.;
const MOUSE_SENSITIVITY: f32 = 0.004;
const MOUSE_ANGVEL_MULTIPLIER: f32 = -75.;
const SPRINT_MULTIPLIER: f32 = 2.;

fn process_controller_inputs(
    mut commands: Commands,
    mut player_query: Query<(
        &ActionState<Actions>,
        &mut FirstPersonController,
        &mut Velocity,
        &Transform,
        Option<&CameraLock>,
        Entity,
    )>,
    mut camera_anchor_query: Query<
        (&mut Transform, &GlobalTransform, Entity),
        (
            Without<FirstPersonController>,
            Without<CameraLock>,
            With<CameraAnchor>,
        ),
    >,
    mut prop_query: Query<
        (
            &Name,
            &GlobalTransform,
            &mut Transform,
            &mut RigidBody,
            &mut CollisionGroups,
        ),
        (Without<FirstPersonController>, Without<CameraAnchor>),
    >,
    rapier: Res<RapierContext>,
) {
    for (input_state, mut controller, mut velocity, transform, yaw_lock, player_entity) in
        &mut player_query
    {
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

            if let Ok((mut camera_transform, _, _)) =
                camera_anchor_query.get_mut(controller.camera_anchor)
            {
                camera_transform.rotation = v_rotation;
            }
        } else {
            velocity.angvel.y = 0.;
        }

        // Grab or release object
        if input_state.just_pressed(Actions::Grab) {
            if controller.grabbed_object.is_none() {
                // Raycast in front of the camera for a prop
                if let Ok((cam_transform, cam_global_transform, camera_entity)) =
                    camera_anchor_query.get_mut(controller.camera_anchor)
                {
                    info!(
                        "Attempting grab from {} towards {}",
                        cam_global_transform.translation(),
                        cam_global_transform.forward()
                    );
                    if let Some((entity, distance)) = rapier.cast_ray(
                        cam_global_transform.translation(),
                        cam_global_transform.forward(),
                        1.5,
                        true,
                        QueryFilter::new().groups(InteractionGroups::new(
                            RAYCAST_GROUP.bits().into(),
                            PROPS_GROUP.bits().into(),
                        )),
                    ) {
                        let (
                            prop_name,
                            _prop_global_transform,
                            mut prop_transform,
                            mut rigidbody,
                            mut collision_groups,
                        ) = prop_query.get_mut(entity).unwrap();
                        info!("Found prop {} to grab {} away!", prop_name, distance);
                        prop_transform.translation = cam_transform.forward() * distance;
                        prop_transform.rotation = Quat::IDENTITY;
                        controller.grabbed_object = Some(entity);
                        *collision_groups = CollisionGroups::new(
                            PROPS_GROUP,
                            WALLS_GROUP | GROUND_GROUP | DOOR_SENSORS_GROUP,
                        );
                        *rigidbody = RigidBody::KinematicPositionBased;
                        commands.entity(camera_entity).add_child(entity);
                    }
                }
            } else {
                // Make the object dynamic again
                let (
                    prop_name,
                    prop_global_transform,
                    mut prop_transform,
                    mut rigidbody,
                    mut collision_groups,
                ) = prop_query
                    .get_mut(controller.grabbed_object.unwrap())
                    .unwrap();
                info!("Releasing prop {}", prop_name);
                *rigidbody = RigidBody::Dynamic;
                commands
                    .entity(player_entity)
                    .remove_children(&[controller.grabbed_object.unwrap()]);
                *collision_groups = CollisionGroups::new(PROPS_GROUP, ALL_GROUPS);
                prop_transform.translation = prop_global_transform.translation();
                controller.grabbed_object = None;
            }
        }
    }
}

fn show_gun_on_pickup(
    mut visibility_query: Query<&mut Visibility>,
    player_query: Query<&FirstPersonController>,
    progress: Res<PlayerProgress>,
) {
    if *progress != PlayerProgress::GettingStarted {
        for controller in &player_query {
            if let Ok(mut visibility) = visibility_query.get_mut(controller.weapon_node) {
                visibility.is_visible = true;
            }
        }
    }
}
