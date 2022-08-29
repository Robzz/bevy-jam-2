use bevy::prelude::*;
use bevy_rapier3d::prelude::{RapierConfiguration, TimestepMode};

pub const WALLS_GROUP: u32 = 0b0000_0001;
pub const PROPS_GROUP: u32 = 0b0000_0010;
pub const PORTAL_GROUP: u32 = 0b0000_0100;
pub const PLAYER_GROUP: u32 = 0b0000_1000;
pub const RAYCAST_GROUP: u32 = 0b0001_0000;
pub const GROUND_GROUP: u32 = 0b0010_0000;
pub const DOOR_SENSORS_GROUP: u32 = 0b0100_0000;
pub const ALL_GROUPS: u32 = 0b0111_1111;

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
