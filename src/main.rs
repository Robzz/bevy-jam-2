#![allow(clippy::type_complexity)]

use bevy::prelude::*;

mod plugins;
mod util;

fn main() {
    App::new().add_plugin(plugins::game::GamePlugin).run();
}
