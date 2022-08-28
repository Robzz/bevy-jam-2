#![allow(dead_code)]

pub mod draw;

use bevy::prelude::PluginGroup;
use bevy_rapier3d::prelude::RapierDebugRenderPlugin;

#[derive(Debug)]
/// Development plugins intended for debug builds use.
pub struct DeveloperPlugins;

impl PluginGroup for DeveloperPlugins {
    fn build(&mut self, group: &mut bevy::app::PluginGroupBuilder) {
        group
            .add(bevy_editor_pls::prelude::EditorPlugin)
            .add(RapierDebugRenderPlugin::default())
            .add(bevy_inspector_egui_rapier::InspectableRapierPlugin);
    }
}
