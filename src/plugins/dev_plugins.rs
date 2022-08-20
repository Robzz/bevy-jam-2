use bevy::prelude::*;

#[derive(Debug)]
/// Development plugins intended for debug builds use.
pub struct DeveloperPlugins;

impl PluginGroup for DeveloperPlugins {
    fn build(&mut self, group: &mut bevy::app::PluginGroupBuilder) {
        group.add(bevy_editor_pls::prelude::EditorPlugin);
    }
}
