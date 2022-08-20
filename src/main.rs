use bevy::prelude::*;

mod plugins;

fn main() {
    App::new()
        .add_plugin(plugins::game::GamePlugin)
        .run();
}
