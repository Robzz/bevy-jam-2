#![allow(clippy::type_complexity)]

use bevy::prelude::*;

mod plugins;
mod util;

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            title: "Lost Portal Prototype v.0.666".to_string(),
            width: 1280.,
            height: 720.,
            ..Default::default()
        })
        .add_plugin(plugins::game::GamePlugin)
        .run();
}
