use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::plugins::physics::*;

/// Setup a test room in a square flate arena format of specified size.
/// 5 cubes for the walls and floor, with physics colliders.
pub fn make_test_arena(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    length: f32,
    height: f32,
) {
    const WALL_THICKNESS: f32 = 1.;

    let walls_materials = [
        materials.add(StandardMaterial::from(Color::RED)),
        materials.add(StandardMaterial::from(Color::GREEN)),
        materials.add(StandardMaterial::from(Color::BLUE)),
        materials.add(StandardMaterial::from(Color::ANTIQUE_WHITE)),
    ];
    let ground_material = materials.add(StandardMaterial::from(Color::DARK_GRAY));

    let half_len = length / 2.;
    let wall_mesh = meshes.add(
        shape::Box {
            min_x: -half_len,
            max_x: half_len,
            min_y: -height / 2.,
            max_y: height / 2.,
            min_z: -WALL_THICKNESS / 2.,
            max_z: WALL_THICKNESS / 2.,
        }
        .into(),
    );
    let ground_mesh = meshes.add(
        shape::Box {
            min_x: -half_len * 1.1,
            max_x: half_len * 1.1,
            min_y: -WALL_THICKNESS / 2.,
            max_y: WALL_THICKNESS / 2.,
            min_z: -half_len * 1.1,
            max_z: half_len * 1.1,
        }
        .into(),
    );

    let mut ground = commands.spawn(PbrBundle {
        mesh: ground_mesh.clone(),
        material: ground_material.clone(),
        transform: Transform::from_xyz(0., -WALL_THICKNESS / 2., 0.),
        ..default()
    });
    ground.insert((
        Name::from("Ground"),
        RigidBody::Fixed,
        Collider::cuboid(half_len * 1.1, WALL_THICKNESS / 2., half_len * 1.1),
        CollisionGroups::new(GROUND_GROUP, ALL_GROUPS),
    ));

    ground.with_children(|parent| {
        for (i, mat) in walls_materials.into_iter().enumerate() {
            let mut transform =
                Transform::from_xyz(0., height / 2., -(half_len + WALL_THICKNESS / 2.));
            transform.rotate_around(
                Vec3::new(0., height / 2., 0.),
                Quat::from_axis_angle(Vec3::Y, i as f32 * FRAC_PI_2),
            );
            parent
                .spawn(PbrBundle {
                    mesh: wall_mesh.clone(),
                    material: mat,
                    transform,
                    ..default()
                })
                .insert((
                    Name::from(format!("Wall_{}", i)),
                    RigidBody::Fixed,
                    Collider::cuboid(half_len, height / 2., WALL_THICKNESS / 2.),
                    CollisionGroups::new(WALLS_GROUP, ALL_GROUPS),
                ));
        }
        parent
            .spawn(PbrBundle {
                mesh: ground_mesh,
                material: ground_material,
                transform: Transform::from_translation(Vec3::Y * height),
                ..default()
            })
            .insert((
                Name::from("Ceiling"),
                RigidBody::Fixed,
                Collider::cuboid(half_len * 1.1, WALL_THICKNESS / 2., half_len * 1.1),
                CollisionGroups::new(GROUND_GROUP, ALL_GROUPS),
            ));
    });
}
