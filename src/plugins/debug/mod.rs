#![allow(dead_code)]

pub mod draw;
pub mod dump;

use bevy::prelude::PluginGroup;

#[derive(Debug)]
/// Development plugins intended for debug builds use.
pub struct DeveloperPlugins;

impl PluginGroup for DeveloperPlugins {
    fn build(&mut self, group: &mut bevy::app::PluginGroupBuilder) {
        group.add(bevy_editor_pls::prelude::EditorPlugin);
    }
}
