#![allow(dead_code)]

pub mod draw;

use bevy::{prelude::*, app::PluginGroupBuilder};
use bevy_rapier3d::prelude::RapierDebugRenderPlugin;

#[derive(Debug)]
/// Development plugins intended for debug builds use.
pub struct DeveloperPlugins;

impl PluginGroup for DeveloperPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<DeveloperPlugins>()
            .add(bevy_editor_pls::prelude::EditorPlugin)
            .add(RapierDebugRenderPlugin::default())
            .add(bevy_inspector_egui_rapier::InspectableRapierPlugin)
            .add(DevelopmentPlugin)
    }
}

pub struct DevelopmentPlugin;

impl Plugin for DevelopmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(bevy_prototype_debug_lines::DebugLinesPlugin::default());
    }
}
