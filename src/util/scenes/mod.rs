use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

/// Setup a test room in a square flate arena format of specified size.
/// 5 cubes for the walls and floor, with physics colliders.
pub fn make_test_arena(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    length: f32,
    height: f32
) {
    const WALL_THICKNESS: f32 = 0.5;

    let walls_material = materials.add(StandardMaterial::from(Color::RED));

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
        .into()
    );
    let ground_mesh = meshes.add(
        shape::Box {
            min_x: -half_len,
            max_x: half_len,
            min_y: -WALL_THICKNESS / 2.,
            max_y: WALL_THICKNESS / 2.,
            min_z: -half_len,
            max_z: half_len,
        }
        .into()
    );

    let mut ground = commands.spawn_bundle(PbrBundle {
        mesh: ground_mesh,
        material: walls_material.clone(),
        transform: Transform::from_xyz(0., -WALL_THICKNESS / 2., 0.),
        ..default()
    });
    ground.insert_bundle((
        Name::from("Ground"),
        RigidBody::Fixed,
        Collider::cuboid(half_len, WALL_THICKNESS / 2., half_len)
    ));

    ground.with_children(|parent| {
        for i in 0..4 {
            let mut transform = Transform::from_xyz(0., height / 2., -(half_len + WALL_THICKNESS / 2.));
            transform.rotate_around(Vec3::new(0., height / 2., 0.), Quat::from_axis_angle(Vec3::Y, i as f32 * FRAC_PI_2));
            parent.spawn_bundle(PbrBundle {
                mesh: wall_mesh.clone(),
                material: walls_material.clone(),
                transform,
                ..default()
            }).insert_bundle((
                Name::from(format!("Wall_{}", i)),
                RigidBody::Fixed,
                Collider::cuboid(half_len, height / 2., WALL_THICKNESS / 2.)
            ));
        }
    });
}
