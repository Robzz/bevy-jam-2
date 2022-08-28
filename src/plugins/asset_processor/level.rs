use bevy::{
    gltf::{Gltf, GltfNode},
    prelude::*, reflect::TypeUuid, utils::HashMap,
};

#[derive(Debug, TypeUuid)]
#[uuid = "731c8e90-b2ea-4f05-b7cd-b694101e5a7c"]
pub struct Level {
    pub(crate) gltf: Handle<Gltf>,
    pub(crate) scene: Handle<Scene>,
    pub(crate) player_spawns: HashMap<String, Handle<GltfNode>>,
    pub(crate) name: String,
}

impl Level {
    pub fn new(
        gltf: Handle<Gltf>,
        scene: Handle<Scene>,
        player_spawns: HashMap<String, Handle<GltfNode>>,
        name: String,
    ) -> Level {
        Level {
            gltf,
            scene,
            player_spawns,
            name,
        }
    }
}
