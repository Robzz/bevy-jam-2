use crate::plugins::*;

use bevy::prelude::*;

#[derive(Debug)]
/// Main game plugin, responsible for loading the other game plugins and bootstrapping the game.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins);

        #[cfg(feature = "devel")] {
            app.add_plugins(dev_plugins::DeveloperPlugins);
        }

        app.add_startup_system(setup);
    }
}

/// Perform game initialization
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(Camera2dBundle::default());
    commands.spawn_bundle(SpriteBundle {
        texture: asset_server.load("icon.png"),
        ..Default::default()
    });
}
