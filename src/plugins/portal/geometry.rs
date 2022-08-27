use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use euclid::Angle;

use crate::plugins::{
    first_person_controller::{CameraLock, FirstPersonController},
    physics::*,
};

use super::{AnimateRoll, PORTAL_MESH_DEPTH};

pub fn adjust_portal_origin_to_obstacles(
    base_location: Vec3,
    impact_normal: Vec3,
    up: Vec3,
    rapier: &Res<RapierContext>,
) -> Vec3 {
    let mut corrected_position = base_location;
    let right = up.cross(impact_normal);
    let left = -right;
    let down = -up;
    if let Some((_entity, distance)) = rapier.cast_ray(
        corrected_position,
        down,
        1.,
        false,
        QueryFilter {
            groups: Some(InteractionGroups::new(
                RAYCAST_GROUP,
                WALLS_GROUP | GROUND_GROUP,
            )),
            ..default()
        },
    ) {
        corrected_position += up * (1. - distance);
    } else if let Some((_entity, distance)) = rapier.cast_ray(
        corrected_position,
        up,
        1.,
        false,
        QueryFilter {
            groups: Some(InteractionGroups::new(
                RAYCAST_GROUP,
                WALLS_GROUP | GROUND_GROUP,
            )),
            ..default()
        },
    ) {
        corrected_position += down * (1. - distance);
    }

    if let Some((_entity, distance)) = rapier.cast_ray(
        corrected_position,
        left,
        1.,
        false,
        QueryFilter {
            groups: Some(InteractionGroups::new(
                RAYCAST_GROUP,
                WALLS_GROUP | GROUND_GROUP,
            )),
            ..default()
        },
    ) {
        corrected_position += right * (1. - distance);
    } else if let Some((_entity, distance)) = rapier.cast_ray(
        corrected_position,
        right,
        1.,
        false,
        QueryFilter {
            groups: Some(InteractionGroups::new(
                RAYCAST_GROUP,
                WALLS_GROUP | GROUND_GROUP,
            )),
            ..default()
        },
    ) {
        corrected_position += left * (1. - distance);
    }
    corrected_position
}

pub fn portal_to_portal(
    render_portal_transform: &Transform,
    linked_portal_transform: &Transform,
) -> Transform {
    let render_clip_to_local =
        Transform::from_translation(render_portal_transform.forward() * PORTAL_MESH_DEPTH);
    let linked_local_to_clip =
        Transform::from_translation(linked_portal_transform.back() * PORTAL_MESH_DEPTH);
    let rot = Transform::from_rotation(Quat::from_rotation_y(PI));
    linked_local_to_clip
        * *linked_portal_transform
        * rot
        * Transform::from_matrix(render_portal_transform.compute_matrix().inverse())
        * render_clip_to_local
}

pub fn adjust_player_camera_on_teleport(
    teleport: &Transform,
    camera_global: &Transform,
    camera_local: &mut Transform,
    player_entity: Entity,
    player: &mut Transform,
    player_controller: &mut FirstPersonController,
) {
    // The camera orientation correction works as follows :
    // * We transform the player normally. We note the new player look direction.
    // * If the root player node is not upright, its orientation is set back to upright (vertical
    // Y, horizontal X)
    // * If we applied an upright correction, we correct the orientation to use the previous look
    // vector.

    //let new_look_vector = teleport.rotation.mul_vec3(camera_global.forward());
    //let exit_to_look = Quat::from_rotation_arc(Vec3::NEG_Z, new_look_vector);
    //let (yaw, pitch, roll) = exit_to_look.to_euler(EulerRot::YXZ);
    *player = *teleport * *player;
    if !player.up().abs_diff_eq(Vec3::Y, 0.001) {
        let new_camera_pos = teleport.mul_vec3(camera_global.translation);
        let new_look_vector = teleport.rotation.mul_vec3(camera_local.forward());
        let target_point = player.translation + new_look_vector;
        player.rotation = Quat::IDENTITY;
        let horiz_plane_look_dir = Vec3::new(new_look_vector.x, 0., new_look_vector.z);
        if horiz_plane_look_dir.length() > 0.001 {
            player.look_at(
                player.translation + horiz_plane_look_dir,
                Vec3::Y,
            );
        }
        let player_mid_plane_look_dir = Vec3::new(0., new_look_vector.y, new_look_vector.z);
        if player_mid_plane_look_dir.length() > 0.001 {
            let player_mid_plane_look_dir = player_mid_plane_look_dir.normalize();
            camera_local.look_at(player.translation + 0.75 * Vec3::Y + player_mid_plane_look_dir, Vec3::Y);
            let pitch = camera_local.forward().dot(Vec3::Y).asin();
            player_controller.pitch = Angle::radians(pitch);
        }
    }
    //let roll_correction = Quat::from_rotation_arc(player.up(), Vec3::Y);

    // Insert an animation to correct the player vertical alignment
    //let final_cam_orientation = roll_correction * player.rotation;
    //commands
    //.entity(player_entity)
    //.insert(AnimateRoll::new(
    //player.rotation,
    //final_cam_orientation,
    //Duration::from_millis(500),
    //))
    //.insert(CameraLock);
}
