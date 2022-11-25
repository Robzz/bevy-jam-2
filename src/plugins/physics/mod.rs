use bevy::prelude::*;
use bevy_rapier3d::prelude::{Group, RapierConfiguration, TimestepMode};

pub const WALLS_GROUP: Group = Group::GROUP_1;
pub const PROPS_GROUP: Group = Group::GROUP_2;
pub const PORTAL_GROUP: Group = Group::GROUP_3;
pub const PLAYER_GROUP: Group = Group::GROUP_4;
pub const RAYCAST_GROUP: Group = Group::GROUP_5;
pub const GROUND_GROUP: Group = Group::GROUP_6;
pub const DOOR_SENSORS_GROUP: Group = Group::GROUP_7;
pub const LEVEL_TRANSITION_SENSORS_GROUP: Group = Group::GROUP_8;
pub const ALL_GROUPS: Group = Group::ALL;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(configure_rapier);
    }
}

fn configure_rapier(mut config: ResMut<RapierConfiguration>) {
    // Extra CCD substeps because them portals can go fast
    config.timestep_mode = TimestepMode::Variable {
        max_dt: 1. / 20.,
        time_scale: 1.,
        substeps: 4,
    }
}
