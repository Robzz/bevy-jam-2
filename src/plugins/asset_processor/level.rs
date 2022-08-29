use bevy::{
    gltf::Gltf,
    prelude::*,
    reflect::{FromReflect, TypeUuid},
    utils::HashMap,
};
use bevy_rapier3d::prelude::*;

use crate::plugins::{doors::Door, first_person_controller::FirstPersonController, portal::Portal};

use super::{level_processor::CurrentLevel, SceneAnimationPlayer};

#[derive(Debug, TypeUuid)]
#[uuid = "731c8e90-b2ea-4f05-b7cd-b694101e5a7c"]
#[allow(dead_code)]
pub struct Level {
    pub(crate) gltf: Handle<Gltf>,
    pub(crate) scene: Handle<Scene>,
    pub(crate) name: String,
}

impl Level {
    pub fn new(gltf: Handle<Gltf>, scene: Handle<Scene>, name: String) -> Level {
        Level { gltf, scene, name }
    }
}

#[derive(Debug, Default, Clone, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct SectionTransition {
    pub target_level: String,
    pub close_door: u32,
    pub open_door: u32,
    pub close_animation: Handle<AnimationClip>,
    pub open_animation: Handle<AnimationClip>,
}

#[derive(Debug, Clone, Reflect)]
pub struct PendingTransition {
    pub source: Entity,
    pub destination: Entity,
    pub open_animation: Handle<AnimationClip>,
    pub timer: Timer,
    pub next_section_name: String,
    pub teleported: bool,
}

#[derive(Debug, Default, Clone, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct SectionStart {
    pub section_name: String,
}

#[derive(Debug, Default, Clone, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct SectionFinish {
    pub section_name: String,
}

#[derive(Debug, Clone, Reflect, FromReflect)]
pub struct Section {
    pub name: String,
    pub spawn_point: Entity,
    pub finish_point: Option<Entity>,
}

#[derive(Debug, Clone, Reflect, FromReflect)]
pub struct SectionTable {
    pub table: HashMap<String, Section>,
}

#[derive(Debug, SystemLabel)]
pub enum SectionTransitionLabels {
    InitiateTransition,
    PerformTransition,
}

pub fn init_section_table(
    mut commands: Commands,
    spawn_points_query: Query<(&SectionStart, Entity)>,
    finish_points_query: Query<(&SectionFinish, Entity)>,
) {
    let spawn_points = HashMap::from_iter(
        spawn_points_query
            .into_iter()
            .map(|(sec, e)| (sec.section_name.clone(), e)),
    );
    let finish_points = HashMap::from_iter(
        finish_points_query
            .into_iter()
            .map(|(sec, e)| (sec.section_name.clone(), e)),
    );

    let mut table = HashMap::new();

    for (name, spawn) in spawn_points {
        let finish = finish_points.get(&name);
        let section = Section {
            name,
            spawn_point: spawn,
            finish_point: finish.cloned(),
        };
        table.insert(section.name.clone(), section);
    }

    let transition_table = dbg!(SectionTable { table });
    commands.insert_resource(transition_table);
}

pub fn initiate_section_transition(
    mut commands: Commands,
    mut animator_query: Query<Option<&mut AnimationPlayer>, With<SceneAnimationPlayer>>,
    mut collisions: EventReader<CollisionEvent>,
    mut transitions_query: Query<(&mut SectionTransition, Entity), Without<Door>>,
    current_level: Res<CurrentLevel>,
    sections: Res<SectionTable>,
) {
    if let Ok(Some(mut animator)) = animator_query.get_single_mut() {
        for collision in collisions.iter() {
            if let CollisionEvent::Started(collider_a, collider_b, _flags) = collision {
                let maybe_sensor_entity = transitions_query
                    .get(*collider_a)
                    .or_else(|_| transitions_query.get(*collider_b))
                    .map(|r| r.1);
                if let Ok(sensor_entity) = maybe_sensor_entity {
                    let (transition, _sensor_entity) =
                        transitions_query.get_mut(sensor_entity).unwrap();
                    info!(
                        "Sensor for transition to level {} activated",
                        transition.target_level
                    );
                    animator.play(transition.close_animation.clone());
                    let section = sections
                        .table
                        .get(&current_level.current_section())
                        .unwrap();
                    let next_section = sections.table.get(&transition.target_level).unwrap();
                    let end = section.finish_point.unwrap();
                    let destination = next_section.spawn_point;
                    commands.insert_resource(PendingTransition {
                        source: end,
                        destination,
                        open_animation: transition.open_animation.clone(),
                        timer: Timer::from_seconds(3., false),
                        next_section_name: transition.target_level.clone(),
                        teleported: false,
                    })
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn perform_section_transition(
    mut commands: Commands,
    mut player_query: Query<(&mut Transform, Entity), With<FirstPersonController>>,
    mut animator_query: Query<(&Name, Option<&mut AnimationPlayer>), With<SceneAnimationPlayer>>,
    mut current_level: ResMut<CurrentLevel>,
    portal_a_query: Query<(Option<Entity>, &Portal<0>)>,
    portal_b_query: Query<(Option<Entity>, &Portal<1>)>,
    global_transform_query: Query<&GlobalTransform>,
    transition: Option<ResMut<PendingTransition>>,
    time: Res<Time>,
) {
    if let Some(mut transition) = transition {
        let (mut player, player_entity) = player_query.single_mut();
        if transition.timer.percent_left() < 0.3 && transition.timer.percent_left() > 0.1 {
            commands
                .entity(player_entity)
                .insert(Ccd::disabled())
                .insert(RigidBody::KinematicPositionBased);
        }
        if transition.timer.percent_left() < 0.2 && !transition.teleported {
            transition.teleported = true;
            // Teleport the player to the destination
            let teleport_trf = global_transform_query
                .get(transition.destination)
                .unwrap()
                .mul_transform(Transform::from_matrix(
                    global_transform_query
                        .get(transition.source)
                        .unwrap()
                        .compute_matrix()
                        .inverse(),
                ))
                .compute_transform();
            *player = teleport_trf.mul_transform(*player);

            // Delete open portals
            //if let (Some(portal_entity), portal_a) = portal_a_query.single() {
                //if let Some(camera) = portal_a.camera {
                    //commands.entity(camera)
                        //.despawn_recursive();
                //}
                //commands.entity(portal_entity)
                    //.despawn_recursive();
            //}
            //if let (Some(portal_entity), portal_b) = portal_b_query.single() {
                //if let Some(camera) = portal_b.camera {
                    //commands.entity(camera)
                        //.despawn_recursive();
                //}
                //commands.entity(portal_entity)
                    //.despawn_recursive();
            //}
        }
        if transition.timer.percent_left() < 0.1 {
            commands
                .entity(player_entity)
                //.insert(Ccd::enabled())
                .insert(RigidBody::Dynamic);
        }
        transition.timer.tick(time.delta());
        if transition.timer.finished() {
            // Trigger the door open animation
            for animator in &animator_query {
                dbg!(animator.0);
            }
            let mut animator = animator_query.single_mut().1.unwrap();
            animator.play(dbg!(transition.open_animation.clone()));

            commands.entity(player_entity).insert(RigidBody::Dynamic);

            // Update ECS section data
            current_level.section = transition.next_section_name.clone();
            commands.remove_resource::<PendingTransition>();
        }
    }
}
