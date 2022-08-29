#![allow(dead_code)]

pub mod draw;

use bevy::prelude::*;
use bevy_rapier3d::prelude::RapierDebugRenderPlugin;

#[derive(Debug)]
/// Development plugins intended for debug builds use.
pub struct DeveloperPlugins;

impl PluginGroup for DeveloperPlugins {
    fn build(&mut self, group: &mut bevy::app::PluginGroupBuilder) {
        group
            .add(bevy_editor_pls::prelude::EditorPlugin)
            .add(RapierDebugRenderPlugin::default())
            .add(bevy_inspector_egui_rapier::InspectableRapierPlugin)
            .add(DevelopmentPlugin);
    }
}

pub struct DevelopmentPlugin;

impl Plugin for DevelopmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(bevy_prototype_debug_lines::DebugLinesPlugin::default())
            .add_startup_system(enable_hot_reload);
    }
}

fn enable_hot_reload(asset_server: Res<AssetServer>) {
    asset_server.watch_for_changes().unwrap()
}
