use bevy::{
    gltf::{Gltf, GltfExtras, GltfNode},
    prelude::*,
    reflect::FromReflect,
    scene::SceneInstance,
    utils::{HashMap, HashSet},
};
use bevy_rapier3d::prelude::*;
use iyes_loopless::prelude::*;
use serde::{Deserialize, Deserializer};

use std::str::FromStr;

use crate::plugins::{
    first_person_controller::*, game::GameState, physics::*, portal::PortalTeleport, doors::{DoorSensor, DoorSidedness, Door},
};

use super::{Level, SpawnState};

#[derive(Debug, Deserialize)]
struct LightExtras {
    #[serde(deserialize_with = "bool_from_string")]
    pub shadows: Option<bool>,
}

pub const LEVEL_LIST: &[&str] = &["Level1"];

pub const PLAYER_SPAWN_SUFFIX: &str = ".player_spawn";
pub const LEVEL_STATIC_GEOMETRY_SUFFIX: &str = ".fixed";
pub const LEVEL_GROUND_GEOMETRY_SUFFIX: &str = ".ground";
pub const LEVEL_DYNAMIC_GEOMETRY_SUFFIX: &str = ".prop";

#[derive(Debug, Deserialize)]
pub(crate) struct MeshExtras {
    #[serde(default)]
    #[serde(deserialize_with = "bool_from_string")]
    visibility: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct NodeExtras {
    #[serde(default)]
    #[serde(deserialize_with = "u32_from_string")]
    door_trigger: Option<u32>,
    #[serde(default)]
    #[serde(deserialize_with = "u32_from_string")]
    door: Option<u32>,
    sidedness: Option<DoorSidedness>
}

fn bool_from_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match bool::from_str(&s) {
        Ok(val) => Ok(Some(val)),
        Err(_) => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(&s),
            &"coerces to bool",
        )),
    }
}

fn u32_from_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    if s.is_none() {
        return Ok(None);
    }
    let s = s.unwrap();
    match u32::from_str(&s) {
        Ok(val) => Ok(Some(val)),
        Err(_) => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(&s),
            &"coerces to u32",
        )),
    }
}

#[derive(Debug, Default, Reflect, FromReflect)]
pub struct CurrentLevel {
    level: Handle<Level>,
    sublevel: String,
}

#[allow(dead_code)]
impl CurrentLevel {
    pub fn get(&self) -> Handle<Level> {
        self.level.clone()
    }

    pub fn current_sublevel(&self) -> String {
        self.sublevel.clone()
    }
}

pub struct LevelProcessor {
    player_entity: Option<Entity>,
    current_level: Option<Handle<Level>>,
    current_level_root: Option<Entity>,
    loaded_levels: HashMap<String, Handle<Level>>,
    loaded_levels_gltfs: HashMap<Handle<Gltf>, Handle<Level>>,
    loading_levels: HashMap<String, Handle<Gltf>>,
    hot_reloaded: HashSet<Handle<Gltf>>,
    spawn_state: SpawnState,
}

impl LevelProcessor {
    pub(crate) fn new() -> LevelProcessor {
        LevelProcessor {
            player_entity: None,
            current_level: None,
            current_level_root: None,
            loaded_levels: HashMap::new(),
            loaded_levels_gltfs: HashMap::new(),
            loading_levels: HashMap::new(),
            hot_reloaded: HashSet::new(),
            spawn_state: SpawnState::Idle,
        }
    }

    pub fn current_level(&self) -> Option<Handle<Level>> {
        self.current_level.clone()
    }

    /// Load a level into memory from a GLTF file.
    pub fn load_level(
        &mut self,
        gltf_level: &str,
        level_name: String,
        asset_server: &Res<AssetServer>,
    ) {
        let level_handle = asset_server.load(gltf_level);
        self.loading_levels.insert(level_name, level_handle);
    }

    pub fn instantiate_level(
        &mut self,
        commands: &mut Commands,
        level_name: &str,
    ) -> Result<(), String> {
        if self.spawn_state != SpawnState::Idle {
            return Err("A level is already being spawned".to_owned());
        }
        if self.loading_levels.contains_key(level_name) {
            return Err("The level asset is currently loading".to_owned());
        }

        if let Some(level) = self.loaded_levels.get(level_name) {
            println!("Level load state transitioned to pending");
            self.spawn_state = SpawnState::Pending(level.to_owned());
            commands.insert_resource(NextState(GameState::Loading));
            commands.remove_resource::<CurrentLevel>();

            Ok(())
        } else {
            Err("The level asset is not loaded".to_owned())
        }
    }

    pub(crate) fn init_level_transition(
        level_manager: Res<LevelProcessor>,
        game_state: Res<CurrentState<GameState>>,
    ) {
        match (&level_manager.spawn_state, &game_state.0) {
            (SpawnState::Pending(_), GameState::Loading) => {}
            (SpawnState::Pending(_), _) => {
                error!("Unexpected game state during state transition")
            }
            (lvlmgr_state, _) => error!(
                "Level manager in unexpected state {:?} during state transition",
                lvlmgr_state
            ),
        }
    }

    pub(crate) fn preprocess_scene(scene: &mut Scene) {
        Self::preprocess_point_lights(scene);
        Self::preprocess_nodes(scene);
        Self::preprocess_meshes(scene);
    }

    /// Process point lights in the scene to adjust the shadows setting.
    pub(crate) fn preprocess_point_lights(scene: &mut Scene) {
        let mut query = scene.world.query::<(&mut PointLight, &GltfExtras)>();
        for (mut light, extras) in query.iter_mut(&mut scene.world) {
            if let Ok(tags) = serde_json::from_str::<LightExtras>(&extras.value) {
                if let Some(true) = tags.shadows {
                    light.shadows_enabled = true;
                }
            }
        }
    }

    /// Modify the visibility components of meshes.
    pub(crate) fn preprocess_meshes(scene: &mut Scene) {
        let mut meshes_query = scene.world.query::<(&Handle<Mesh>, &Parent, Entity)>();
        let mut extras_map = HashMap::new();
        for (_mesh, parent, id) in meshes_query.iter(&scene.world) {
            let parent = scene.world.entity(**parent);
            if let Some(extras) = parent.get::<GltfExtras>() {
                match serde_json::from_str::<MeshExtras>(&extras.value) {
                    Ok(mesh_extras) => {
                        extras_map.insert(id, mesh_extras);
                    }
                    Err(e) => println!("Deserializer error: {}", e),
                }
            }
        }

        for (id, extras) in extras_map {
            let mut entity = scene.world.entity_mut(id);

            if let Some(visibility) = extras.visibility {
                entity.insert(Visibility {
                    is_visible: visibility,
                });
            }
        }
    }

    /// Modify the visibility components of nodes and add door trigger components.
    pub(crate) fn preprocess_nodes(scene: &mut Scene) {
        let mut nodes_query = scene.world.query_filtered::<(&GltfExtras, Entity), (With<Transform>, Without<Handle<Mesh>>)>();
        let mut extras_map = HashMap::new();
        for (extras, id) in nodes_query.iter(&scene.world) {
            match serde_json::from_str::<NodeExtras>(&extras.value) {
                Ok(node_extras) => {
                    extras_map.insert(id, node_extras);
                }
                Err(e) => warn!("Deserializer error: {}", e),
            }
        }

        for (id, extras) in extras_map {
            let mut entity = scene.world.entity_mut(id);

            if let Some(door_trigger) = extras.door_trigger {
                entity.insert(DoorSensor { doors_id: door_trigger, door_entities: Vec::new() });
            }

            if let (Some(door_id), Some(door_sidedness)) = (extras.door, extras.sidedness) {
                entity.insert(Door { id: door_id, sidedness: door_sidedness  });
            }
        }
    }

    pub(crate) fn gltf_asset_event_listener(
        mut level_manager: ResMut<LevelProcessor>,
        mut scenes: ResMut<Assets<Scene>>,
        mut gltfs: ResMut<Assets<Gltf>>,
        mut events: EventReader<AssetEvent<Gltf>>,
    ) {
        for event in events.iter() {
            match event {
                AssetEvent::Created { handle: _ } => {}
                AssetEvent::Modified { handle } => {
                    if level_manager.hot_reloaded.contains(handle) {
                        // Asset was just hot reloaded, this event is from our own modifications, discard it.
                        level_manager.hot_reloaded.remove(handle);
                        continue;
                    }
                    if let Some(_level) = level_manager.loaded_levels_gltfs.get(handle) {
                        let mut gltf = gltfs.get_mut(handle).unwrap();
                        Self::update_level_on_gltf_reload(&mut scenes, &mut gltf);
                        level_manager.hot_reloaded.insert(handle.to_owned());
                    }
                }
                AssetEvent::Removed { handle: _ } => {}
            }
        }
    }

    pub(crate) fn postprocess_scene(
        mut commands: Commands,
        scene_spawner: Res<SceneSpawner>,
        meshes: Res<Assets<Mesh>>,
        mut level_manager: ResMut<LevelProcessor>,
        fixed_geometry_query: Query<(&Name, &Handle<Mesh>, Entity)>,
        dynamic_geometry_query: Query<(&Name, &Children, Entity)>,
        doors_query: Query<(&Name, &Door, Entity)>,
        mut door_sensors_query: Query<(&Name, &mut DoorSensor, &Children, Entity)>,
        scene_instance_query: Query<&SceneInstance>,
    ) {
        if let SpawnState::ProcessingScene(scene_entity) = level_manager.spawn_state {
            if let Ok(scene_id) = scene_instance_query.get(scene_entity) {
                if scene_spawner.instance_is_ready(**scene_id) {
                    let mut colliders = HashMap::new();
                    let mut doors = HashMap::new();
                    for scene_entity in scene_spawner.iter_instance_entities(**scene_id).unwrap() {
                        for (name, mesh_handle, entity) in fixed_geometry_query.get(scene_entity) {
                            if name.ends_with(LEVEL_STATIC_GEOMETRY_SUFFIX) {
                                let mesh = meshes.get(mesh_handle).unwrap();

                                commands.entity(entity).insert_bundle((
                                    CollisionGroups::new(WALLS_GROUP, ALL_GROUPS),
                                    RigidBody::Fixed,
                                    Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh)
                                        .unwrap(),
                                ));
                            } else if name.ends_with(LEVEL_GROUND_GEOMETRY_SUFFIX) {
                                let mesh = meshes.get(mesh_handle).unwrap();

                                commands.entity(entity).insert_bundle((
                                    CollisionGroups::new(GROUND_GROUP, ALL_GROUPS),
                                    RigidBody::Fixed,
                                    Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh)
                                        .unwrap(),
                                ));
                            }
                        }

                        for (name, children, entity) in dynamic_geometry_query.get(scene_entity) {
                            if name.ends_with(LEVEL_DYNAMIC_GEOMETRY_SUFFIX) {
                                if let Ok((_name, mesh_handle, _entity)) =
                                    fixed_geometry_query.get(*children.first().unwrap())
                                {
                                    let mesh = meshes.get(mesh_handle).unwrap();
                                    let collider = colliders
                                        .entry(mesh_handle.id)
                                        .or_insert_with(|| Self::compute_nonconvex_collider(mesh));
                                    commands.entity(entity).insert_bundle((
                                        CollisionGroups::new(PROPS_GROUP, ALL_GROUPS),
                                        RigidBody::Dynamic,
                                        Velocity::default(),
                                        ColliderMassProperties::Density(200.),
                                        collider.clone(),
                                        PortalTeleport,
                                    ));
                                } else {
                                    warn!("Dynamic geometry node without a child mesh");
                                }
                            }
                        }

                        for (name, door, entity) in doors_query.get(scene_entity) {
                            doors.entry(door.id).or_insert_with(Vec::new).push(entity);
                            info!("Got door {}: {:?}", name, door);
                        }

                        for (name, mut sensor, children, entity) in door_sensors_query.get_mut(scene_entity) {
                            info!("Got door sensor {}: {:?}", name, sensor);
                            if let Ok((_, mesh_handle, _)) = fixed_geometry_query.get(*children.first().unwrap()) {
                                let mesh = meshes.get(mesh_handle).unwrap();
                                commands.entity(entity)
                                    .insert_bundle((
                                        RigidBody::Fixed,
                                        Collider::from_bevy_mesh(&mesh, &ComputedColliderShape::TriMesh).unwrap(),
                                        Sensor
                                    ));
                                if let Some(sensor_doors) = doors.get(&sensor.doors_id) {
                                    sensor.door_entities = sensor_doors.clone()
                                }
                                else {
                                    warn!("No doors found for sensor {} with ID {}", name, sensor.doors_id);
                                }
                            }
                        }
                    }

                    info!("Level geometry processed");
                    level_manager.spawn_state = SpawnState::Spawning;
                    level_manager.current_level_root = Some(scene_entity);
                }
            }
        }
    }

    pub(crate) fn spawn_level_system(
        mut commands: Commands,
        mut level_manager: ResMut<LevelProcessor>,
        levels: Res<Assets<Level>>,
    ) {
        if let SpawnState::Pending(level_handle) = &level_manager.spawn_state {
            println!("Spawning level scene");
            let level = levels.get(level_handle).unwrap();

            if let Some(current_level_root) = level_manager.current_level_root {
                commands.entity(current_level_root).despawn_recursive();
            }

            if let Some(player) = level_manager.player_entity {
                commands.entity(player).despawn_recursive();
            }

            let scene_instance = commands
                .spawn_bundle(SceneBundle {
                    scene: level.scene.clone(),
                    ..default()
                })
                .id();
            level_manager.current_level = Some(level_handle.to_owned());
            println!("Level load state transitioned to waiting for scene processing");
            level_manager.spawn_state = SpawnState::ProcessingScene(scene_instance);
        }
    }

    pub(crate) fn check_level_loading_progress(
        mut level_manager: ResMut<LevelProcessor>,
        mut levels: ResMut<Assets<Level>>,
        mut scenes: ResMut<Assets<Scene>>,
        mut gltfs: ResMut<Assets<Gltf>>,
        asset_server: Res<AssetServer>,
    ) {
        if !level_manager.loading_levels.is_empty() {
            let mut loaded_levels = Vec::new();

            for (level_name, level_gltf) in &level_manager.loading_levels {
                match asset_server.get_load_state(level_gltf) {
                    bevy::asset::LoadState::Loaded => {
                        let mut gltf = gltfs
                            .get_mut(level_gltf)
                            .expect("Wasn't able to obtain GLTF though Bevy says it's loaded");
                        let level = Self::process_gltf_levels(
                            &mut levels,
                            &mut scenes,
                            &mut gltf,
                            level_gltf,
                            level_name,
                        );
                        loaded_levels.push((level_name.to_owned(), level, level_gltf.to_owned()));
                    }
                    _ => {}
                }
            }

            for (level_name, handle, gltf) in loaded_levels {
                level_manager.loading_levels.remove(&level_name);
                level_manager
                    .loaded_levels
                    .insert(level_name, handle.clone());
                level_manager.loaded_levels_gltfs.insert(gltf, handle);
            }
        }
    }

    pub(crate) fn spawn_player(
        mut commands: Commands,
        mut level_manager: ResMut<LevelProcessor>,
        levels: Res<Assets<Level>>,
        nodes: Res<Assets<GltfNode>>,
    ) {
        if level_manager.spawn_state == SpawnState::Spawning {
            let level = levels.get(&level_manager.current_level().unwrap()).unwrap();
            let spawn_node = nodes.get(&level.player_spawns[LEVEL_LIST[0]]).unwrap();
            let player_entity = commands
                .spawn_bundle(FirstPersonControllerBundle {
                    spawner: FirstPersonControllerSpawner {},
                    spatial: SpatialBundle {
                        transform: spawn_node.transform.clone(),
                        ..default()
                    },
                })
                .insert(PortalTeleport)
                .id();

            level_manager.player_entity = Some(player_entity);
            level_manager.spawn_state = SpawnState::Finalizing;
            commands.insert_resource(NextState(GameState::InGame));
        }
    }

    pub(crate) fn finalize_level_spawn(
        mut commands: Commands,
        mut level_manager: ResMut<LevelProcessor>,
    ) {
        if level_manager.spawn_state == SpawnState::Finalizing
            && level_manager.current_level_root.is_some()
            && level_manager.player_entity.is_some()
        {
            info!("Marking level spawn as complete, transitioning to in game state");
            commands.insert_resource(CurrentLevel {
                level: level_manager.current_level().unwrap(),
                sublevel: "Level1".to_owned(),
            });
            level_manager.spawn_state = SpawnState::Idle;
        }
    }

    // Private methods
    fn process_gltf_levels(
        levels: &mut ResMut<Assets<Level>>,
        scenes: &mut ResMut<Assets<Scene>>,
        gltf: &mut Gltf,
        handle: &Handle<Gltf>,
        level_name: &str,
    ) -> Handle<Level> {
        let mut spawn_nodes = HashMap::new();
        for (name, node) in &gltf.named_nodes {
            if name.ends_with(PLAYER_SPAWN_SUFFIX) {
                spawn_nodes.insert(
                    name.strip_suffix(PLAYER_SPAWN_SUFFIX).unwrap().to_owned(),
                    node.clone(),
                );
            }
        }
        if spawn_nodes.is_empty() {
            panic!("The level has no player spawn point");
        }
        let default_scene_handle = gltf.default_scene.as_ref().unwrap();
        let mut default_scene = scenes.get_mut(default_scene_handle).unwrap();

        // Add required components to the entities in the scene's world based on the GltfExtras
        LevelProcessor::preprocess_scene(&mut default_scene);
        let level = Level::new(
            handle.to_owned(),
            // No need for strong handles if we're keeping a handle to the level besides the
            // weak refs to level elements
            gltf.default_scene
                .as_ref()
                .expect("GLTF asset has no default scene")
                .as_weak(),
            spawn_nodes,
            level_name.to_owned(),
        );
        levels.add(level)
    }

    fn update_level_on_gltf_reload(scenes: &mut ResMut<Assets<Scene>>, gltf: &mut Gltf) {
        let default_scene_handle = gltf.default_scene.as_ref().unwrap();
        let mut default_scene = scenes.get_mut(default_scene_handle).unwrap();
        LevelProcessor::preprocess_scene(&mut default_scene);
    }

    fn compute_nonconvex_collider(mesh: &Mesh) -> Collider {
        Collider::from_bevy_mesh(
            mesh,
            &ComputedColliderShape::ConvexDecomposition(VHACDParameters::default()),
        )
        .unwrap()
    }
}
