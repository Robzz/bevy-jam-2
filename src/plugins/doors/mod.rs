use bevy::{prelude::*, reflect::FromReflect, utils::HashSet};
use bevy_rapier3d::prelude::*;
use serde::Deserialize;

use super::asset_processor::SceneAnimationPlayer;

pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Door>()
            .register_type::<DoorSensor>()
            .register_type::<DoorSidedness>()
            .add_system(open_doors_on_sensor_activation);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Reflect, FromReflect)]
pub enum DoorSidedness {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct Door {
    pub id: u32,
    pub sidedness: DoorSidedness,
    pub open: bool,
    pub animation_open: Handle<AnimationClip>,
    pub animation_close: Handle<AnimationClip>,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct DoorSensor {
    pub doors_id: u32,
    pub door_entities: Vec<Entity>,
    pub active_collisions: HashSet<Entity>,
}

#[derive(Debug, Default)]
pub struct DoorAnimations {
    pub close_left: Handle<AnimationClip>,
    pub close_right: Handle<AnimationClip>,
    pub open_left: Handle<AnimationClip>,
    pub open_right: Handle<AnimationClip>,
}

fn open_doors_on_sensor_activation(
    mut animator_query: Query<Option<&mut AnimationPlayer>, With<SceneAnimationPlayer>>,
    mut doors_query: Query<&mut Door>,
    mut collisions: EventReader<CollisionEvent>,
    mut sensor_query: Query<(&mut DoorSensor, Entity), Without<Door>>,
) {
    if let Ok(Some(mut animator)) = animator_query.get_single_mut() {
        for collision in collisions.iter() {
            match collision {
                CollisionEvent::Started(collider_a, collider_b, _flags) => {
                    let maybe_sensor_entity = sensor_query
                        .get(*collider_a)
                        .or_else(|_| sensor_query.get(*collider_b))
                        .map(|r| r.1);
                    if let Ok(sensor_entity) = maybe_sensor_entity {
                        let (mut sensor, sensor_entity) = sensor_query.get_mut(sensor_entity).unwrap();
                        let cause = if &sensor_entity == collider_a {
                            *collider_b
                        } else {
                            *collider_a
                        };
                        if sensor.active_collisions.is_empty() {
                            info!(
                                "Sensor for door {} activated, opening door entities {:?}",
                                sensor.doors_id, &sensor.door_entities
                            );
                            for entity in &sensor.door_entities {
                                let mut door = doors_query.get_mut(*entity).unwrap();
                                animator.play(door.animation_open.clone());
                                door.open = true;
                            }
                        }
                        sensor.active_collisions.insert(cause);
                    }
                }
                CollisionEvent::Stopped(collider_a, collider_b, _flags) => {
                    let maybe_sensor_entity = sensor_query
                        .get(*collider_a)
                        .or_else(|_| sensor_query.get(*collider_b))
                        .map(|r| r.1);
                    if let Ok(sensor_entity) = maybe_sensor_entity {
                        let (mut sensor, sensor_entity) = sensor_query.get_mut(sensor_entity).unwrap();
                        let cause = if &sensor_entity == collider_a {
                            *collider_b
                        } else {
                            *collider_a
                        };
                        sensor.active_collisions.remove(&cause);
                        if sensor.active_collisions.is_empty() {
                            info!(
                                "Sensor for door {} deactivated, closin door entities {:?}",
                                sensor.doors_id, &sensor.door_entities
                            );
                            for entity in &sensor.door_entities {
                                let mut door = doors_query.get_mut(*entity).unwrap();
                                animator.play(door.animation_close.clone());
                                door.open = false;
                            }
                        }
                    }
                }
            }
        }
    }
}
