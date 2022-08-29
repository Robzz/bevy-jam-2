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
    doors::{Door, DoorSensor},
    first_person_controller::*,
    game::GameState,
    physics::*,
    portal::PortalTeleport,
    render::RenderResources,
};

use super::{Level, SpawnState};

pub const LEVEL_LIST: &[&str] = &["Level1"];

pub const PLAYER_SPAWN_SUFFIX: &str = ".player_spawn";
pub const LEVEL_STATIC_GEOMETRY_SUFFIX: &str = ".fixed";
pub const LEVEL_GROUND_GEOMETRY_SUFFIX: &str = ".ground";
pub const LEVEL_DYNAMIC_GEOMETRY_SUFFIX: &str = ".prop";
pub const ANIMATION_OPEN_DOOR_PREFIX: &str = "OpenDoor";
pub const ANIMATION_CLOSE_DOOR_PREFIX: &str = "CloseDoor";

#[derive(Debug, Component, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct SceneAnimationPlayer;

#[derive(Debug, Deserialize)]
struct LightExtras {
    #[serde(deserialize_with = "bool_from_string")]
    pub shadows: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MeshExtras {
    #[serde(default)]
    #[serde(deserialize_with = "bool_from_string")]
    visibility: Option<bool>,
    #[serde(default)]
    #[serde(deserialize_with = "bool_from_string")]
    grid: Option<bool>,
    shape: Option<ColliderShape>,
}

#[derive(Debug, Component, Clone, Deserialize, Default, Reflect, FromReflect)]
#[reflect(Component)]
#[serde(rename_all = "snake_case")]
pub enum ColliderShape {
    #[default]
    Convex,
    Concave,
}

#[derive(Debug, Clone, Deserialize, Default, Reflect, FromReflect)]
enum ExtrasAlphaMode {
    #[default]
    Opaque,
    Blend,
}

impl From<ExtrasAlphaMode> for AlphaMode {
    fn from(alpha: ExtrasAlphaMode) -> Self {
        match alpha {
            ExtrasAlphaMode::Opaque => AlphaMode::Opaque,
            ExtrasAlphaMode::Blend => AlphaMode::Blend,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MaterialExtras {
    #[serde(default)]
    alpha: Option<ExtrasAlphaMode>,
}

#[derive(Debug, Deserialize)]
pub struct NodeExtras {
    #[serde(default)]
    #[serde(deserialize_with = "u32_from_string")]
    door_trigger: Option<u32>,
    #[serde(default)]
    #[serde(deserialize_with = "u32_from_string")]
    door: Option<u32>,
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

    /// Modify the alpha blending attribute of standard materials.
    pub(crate) fn preprocess_materials(
        scene: &mut Scene,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) {
        let mut query = scene
            .world
            .query::<(&Handle<StandardMaterial>, &GltfExtras)>();
        for (material_handle, extras) in query.iter(&scene.world) {
            if let Ok(tags) = serde_json::from_str::<MaterialExtras>(&extras.value) {
                if let Some(alpha) = tags.alpha {
                    let material = materials.get_mut(material_handle).unwrap();
                    material.alpha_mode = alpha.into();
                }
            }
        }
    }

    /// Modify the visibility components of meshes.
    pub(crate) fn preprocess_meshes(scene: &mut Scene, grids: &Res<RenderResources>) {
        let mut meshes_query = scene.world.query::<(&Handle<Mesh>, &Parent, Entity)>();
        let mut extras_map = HashMap::new();
        for (_mesh, parent, id) in meshes_query.iter(&scene.world) {
            let parent = scene.world.entity(**parent);
            if let Some(extras) = parent.get::<GltfExtras>() {
                match serde_json::from_str::<MeshExtras>(&extras.value) {
                    Ok(mesh_extras) => {
                        extras_map.insert(id, mesh_extras);
                    }
                    Err(e) => warn!("Deserializer error: {}", e),
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

            if let Some(true) = extras.grid {
                entity.remove::<Handle<StandardMaterial>>();
                entity.insert(grids.default_grid_material.clone());
            }

            entity.insert(extras.shape.unwrap_or_default());
        }
    }

    /// Modify the visibility components of nodes and add door trigger components.
    pub(crate) fn preprocess_nodes(scene: &mut Scene, gltf: &Gltf) {
        let mut nodes_query = scene
            .world
            .query_filtered::<(&GltfExtras, Entity), (With<Transform>, Without<Handle<Mesh>>)>();
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
                entity.insert(DoorSensor {
                    doors_id: door_trigger,
                    door_entities: Vec::new(),
                    ..default()
                });
            }

            if let Some(door_id) = extras.door {
                entity.insert(Door {
                    id: door_id,
                    animation_open: gltf
                        .named_animations
                        .get(&format!("{}_{}", ANIMATION_OPEN_DOOR_PREFIX, door_id))
                        .unwrap()
                        .clone(),
                    animation_close: gltf
                        .named_animations
                        .get(&format!("{}_{}", ANIMATION_CLOSE_DOOR_PREFIX, door_id))
                        .unwrap()
                        .clone(),
                    ..default()
                });
            }
        }

        let mut animator_query = scene
            .world
            .query_filtered::<Entity, With<AnimationPlayer>>();
        let animator_entity = animator_query.single(&scene.world);
        scene
            .world
            .entity_mut(animator_entity)
            .insert(SceneAnimationPlayer);
    }

    pub(crate) fn gltf_asset_event_listener(
        mut level_manager: ResMut<LevelProcessor>,
        mut scenes: ResMut<Assets<Scene>>,
        mut gltfs: ResMut<Assets<Gltf>>,
        mut events: EventReader<AssetEvent<Gltf>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        grids: Res<RenderResources>,
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
                        let gltf = gltfs.get_mut(handle).unwrap();
                        Self::update_level_on_gltf_reload(
                            &mut scenes,
                            &mut materials,
                            &grids,
                            gltf,
                        );
                        level_manager.hot_reloaded.insert(handle.to_owned());
                    }
                }
                AssetEvent::Removed { handle: _ } => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn postprocess_scene(
        mut commands: Commands,
        mut level_manager: ResMut<LevelProcessor>,
        mut door_sensors_query: Query<(&Name, &mut DoorSensor, &Children, Entity)>,
        fixed_geometry_query: Query<(&Name, &Handle<Mesh>, Option<&ColliderShape>, Entity)>,
        dynamic_geometry_query: Query<(&Name, &Children, Entity)>,
        doors_query: Query<(&Name, &Door, Entity)>,
        scene_instance_query: Query<&SceneInstance>,
        scene_spawner: Res<SceneSpawner>,
        meshes: Res<Assets<Mesh>>,
    ) {
        if let SpawnState::ProcessingScene(scene_entity) = level_manager.spawn_state {
            if let Ok(scene_id) = scene_instance_query.get(scene_entity) {
                if scene_spawner.instance_is_ready(**scene_id) {
                    let mut colliders = HashMap::new();
                    let mut doors = HashMap::new();
                    let mut sensors = Vec::new();
                    for scene_entity in scene_spawner.iter_instance_entities(**scene_id).unwrap() {
                        if let Ok((name, mesh_handle, opt_shape, entity)) =
                            fixed_geometry_query.get(scene_entity)
                        {
                            let shape = opt_shape.cloned().unwrap_or_default();
                            if name.ends_with(LEVEL_STATIC_GEOMETRY_SUFFIX) {
                                let mesh = meshes.get(mesh_handle).unwrap();

                                dbg!(&shape, name);
                                commands.entity(entity).insert_bundle((
                                    CollisionGroups::new(
                                        WALLS_GROUP,
                                        ALL_GROUPS - DOOR_SENSORS_GROUP,
                                    ),
                                    RigidBody::Fixed,
                                    Self::compute_collider(mesh, shape),
                                ));
                            } else if name.ends_with(LEVEL_GROUND_GEOMETRY_SUFFIX) {
                                let mesh = meshes.get(mesh_handle).unwrap();

                                dbg!(&shape, name);
                                commands.entity(entity).insert_bundle((
                                    CollisionGroups::new(
                                        GROUND_GROUP,
                                        ALL_GROUPS - DOOR_SENSORS_GROUP,
                                    ),
                                    RigidBody::Fixed,
                                    Self::compute_collider(mesh, shape),
                                ));
                            }
                        }

                        if let Ok((name, children, entity)) =
                            dynamic_geometry_query.get(scene_entity)
                        {
                            if name.ends_with(LEVEL_DYNAMIC_GEOMETRY_SUFFIX) {
                                if let Ok((_name, mesh_handle, _opt_shape, _entity)) =
                                    fixed_geometry_query.get(*children.first().unwrap())
                                {
                                    let mesh = meshes.get(mesh_handle).unwrap();
                                    let collider =
                                        colliders.entry(mesh_handle.id).or_insert_with(|| {
                                            Self::compute_collider(mesh, ColliderShape::Concave)
                                        });
                                    //.or_insert_with(|| Self::compute_collider(mesh, opt_shape.cloned().unwrap_or(ColliderShape::Concave)));
                                    commands.entity(entity).insert_bundle((
                                        CollisionGroups::new(PROPS_GROUP, ALL_GROUPS),
                                        RigidBody::Dynamic,
                                        Velocity::default(),
                                        ColliderMassProperties::Density(200.),
                                        Ccd::enabled(),
                                        collider.clone(),
                                        PortalTeleport,
                                    ));
                                } else {
                                    warn!("Dynamic geometry node without a child mesh");
                                }
                            }
                        }

                        if let Ok((_name, door, entity)) = doors_query.get(scene_entity) {
                            doors.entry(door.id).or_insert_with(Vec::new).push(entity);
                        }

                        if let Ok((_name, _sensor, children, entity)) =
                            door_sensors_query.get_mut(scene_entity)
                        {
                            if let Ok((_, mesh_handle, opt_shape, _)) =
                                fixed_geometry_query.get(*children.first().unwrap())
                            {
                                let mesh = meshes.get(mesh_handle).unwrap();
                                let shape = opt_shape.cloned().unwrap_or_default();
                                commands.entity(entity).insert_bundle((
                                    RigidBody::Fixed,
                                    Self::compute_collider(mesh, shape),
                                    Sensor,
                                    CollisionGroups::new(
                                        DOOR_SENSORS_GROUP,
                                        PLAYER_GROUP | PROPS_GROUP,
                                    ),
                                    ActiveEvents::COLLISION_EVENTS,
                                ));
                                sensors.push(entity);
                            }
                        }
                    }

                    for sensor_entity in sensors {
                        if let Ok((name, mut sensor, _, _)) =
                            door_sensors_query.get_mut(sensor_entity)
                        {
                            if let Some(sensor_doors) = doors.get(&sensor.doors_id) {
                                sensor.door_entities = sensor_doors.clone()
                            } else {
                                dbg!(&doors);
                                warn!(
                                    "No doors found for sensor {} with ID {}",
                                    name, sensor.doors_id
                                );
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
        mut materials: ResMut<Assets<StandardMaterial>>,
        grid_materials: Res<RenderResources>,
        asset_server: Res<AssetServer>,
    ) {
        if !level_manager.loading_levels.is_empty() {
            let mut loaded_levels = Vec::new();

            for (level_name, level_gltf) in &level_manager.loading_levels {
                if asset_server.get_load_state(level_gltf) == bevy::asset::LoadState::Loaded {
                    let gltf = gltfs
                        .get_mut(level_gltf)
                        .expect("Wasn't able to obtain GLTF though Bevy says it's loaded");
                    let level = Self::process_gltf_levels(
                        &mut levels,
                        &mut scenes,
                        &mut materials,
                        gltf,
                        level_gltf,
                        level_name,
                        &grid_materials,
                    );
                    loaded_levels.push((level_name.to_owned(), level, level_gltf.to_owned()));
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
                        transform: spawn_node.transform,
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
        materials: &mut ResMut<Assets<StandardMaterial>>,
        gltf: &mut Gltf,
        handle: &Handle<Gltf>,
        level_name: &str,
        grids: &Res<RenderResources>,
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
        let default_scene = scenes.get_mut(default_scene_handle).unwrap();

        // Add required components to the entities in the scene's world based on the GltfExtras
        Self::preprocess_point_lights(default_scene);
        Self::preprocess_nodes(default_scene, gltf);
        Self::preprocess_meshes(default_scene, grids);
        Self::preprocess_materials(default_scene, materials);
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

    fn update_level_on_gltf_reload(
        scenes: &mut ResMut<Assets<Scene>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        grids: &Res<RenderResources>,
        gltf: &mut Gltf,
    ) {
        let default_scene_handle = gltf.default_scene.as_ref().unwrap();
        let default_scene = scenes.get_mut(default_scene_handle).unwrap();
        Self::preprocess_point_lights(default_scene);
        Self::preprocess_nodes(default_scene, gltf);
        Self::preprocess_meshes(default_scene, grids);
        Self::preprocess_materials(default_scene, materials);
    }

    fn compute_collider(mesh: &Mesh, shape: ColliderShape) -> Collider {
        Collider::from_bevy_mesh(
            mesh,
            &match shape {
                ColliderShape::Convex => ComputedColliderShape::TriMesh,
                ColliderShape::Concave => {
                    let vhacd_params = VHACDParameters {
                        fill_mode: FillMode::FloodFill { detect_cavities: true },
                        convex_hull_approximation: true,
                        resolution: 128,
                        ..default()
                    };
                    ComputedColliderShape::ConvexDecomposition(vhacd_params)
                }
            },
        )
        .unwrap()
    }
}
