use bevy::{prelude::*, reflect::FromReflect};
use serde::Deserialize;


pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Door>();
        app.register_type::<DoorSensor>();
        app.register_type::<DoorSidedness>();
    }
}

#[derive(Debug, Clone, Default, Deserialize, Reflect, FromReflect)]
pub enum DoorSidedness {
    Left,
    Right,
    #[default]
    None
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct Door {
    pub id: u32,
    pub sidedness: DoorSidedness,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
#[reflect(Component)]
pub struct DoorSensor {
    pub doors_id: u32,
    pub door_entities: Vec<Entity>
}
