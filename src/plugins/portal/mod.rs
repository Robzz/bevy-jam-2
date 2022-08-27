//! This module defines portals that can be placed throughout the world to travel between linked
//! portals, as well as render from one another's viewpoint.
//!
//! Portal conventions:
//!
//! * The portal origin is at the center of the portal volume.
//! * The portal clipping plane defined as the portal *back*.

use std::{f32::consts::FRAC_PI_4, time::Duration};

use bevy::{
    math::{Vec3Swizzles, Vec4Swizzles},
    prelude::*,
    reflect::FromReflect,
    render::{
        camera::{Projection, RenderTarget},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
    transform::TransformSystem,
};
use bevy_prototype_debug_lines::DebugLines;
use bevy_rapier3d::prelude::*;

mod camera_projection;
mod geometry;
mod material;

use camera_projection::PortalCameraProjection;
use material::*;
use noise::{
    utils::{NoiseMapBuilder, PlaneMapBuilder},
    Fbm,
};

use super::{first_person_controller::*, physics::*};

#[derive(Debug)]
pub struct PortalPlugin;

// TODO:
//
// * Transition between the open and closed materials depending on whether there are 1 or 2 portals open
// * Figure where to place the portal cameras
//   * Same thing for recursive portal iterations

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<OpenPortalMaterial>::default())
            .add_plugin(MaterialPlugin::<ClosedPortalMaterial>::default())
            .register_type::<Portal<0>>()
            .register_type::<Portal<1>>()
            .register_type::<PortalOrientation>()
            .register_type::<PortalResources>()
            .register_type::<OpenPortalMaterial>()
            .register_type::<ClosedPortalMaterial>()
            .register_type::<PortalTeleport>()
            .add_plugin(bevy::render::camera::CameraProjectionPlugin::<
                PortalCameraProjection,
            >::default())
            .add_startup_system(load_portal_assets)
            .add_system(ClosedPortalMaterial::update_time_uniform)
            .add_system(set_portal_materials)
            .add_system(update_main_camera.label(PortalLabels::UpdateMainCamera))
            .add_system_set(
                SystemSet::new()
                    .label(PortalLabels::ShootPortals)
                    .with_system(fire_portal::<0, 1>)
                    .with_system(fire_portal::<1, 0>),
            )
            .add_system_set(
                SystemSet::new()
                    .label(PortalLabels::CreateCameras)
                    .after(PortalLabels::ShootPortals)
                    .after(PortalLabels::UpdateMainCamera)
                    .with_system(create_portal_cameras::<0>)
                    .with_system(create_portal_cameras::<1>),
            )
            .add_system_set(
                SystemSet::new()
                    .label(PortalLabels::SyncCameras)
                    .after(PortalLabels::CreateCameras)
                    .with_system(sync_portal_cameras),
            )
            .add_system(
                turn_off_collisions_with_static_geo_when_in_portal.after(PortalLabels::SyncCameras),
            )
            .add_system_set(
                SystemSet::new()
                    .with_system(teleport_props)
                    .with_system(teleport_player)
                    .label(PortalLabels::TeleportEntities)
                    .after(PortalLabels::SyncCameras),
            )
            .add_system(
                animate_camera_roll
                    .label(PortalLabels::AnimateCamera)
                    .after(PortalLabels::TeleportEntities),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                bevy::render::view::update_frusta::<PortalCameraProjection>
                    .after(TransformSystem::TransformPropagate),
            );
    }
}

impl PortalPlugin {
    fn spawn_portal<const N: u32>(
        commands: &mut Commands,
        player_transform: &GlobalTransform,
        portal_query: &Query<(&Portal<N>, Entity)>,
        other_portal_entity: Option<Entity>,
        rapier: &Res<RapierContext>,
        portal_res: &Res<PortalResources>,
    ) -> Option<Entity> {
        let (_entity, impact) = rapier.cast_ray_and_get_normal(
            player_transform.translation(),
            player_transform.forward(),
            Real::MAX,
            true,
            QueryFilter::only_fixed().groups(InteractionGroups::new(
                RAYCAST_GROUP,
                WALLS_GROUP | GROUND_GROUP,
            )),
        )?;

        if let Ok((previous_portal, entity)) = portal_query.get_single() {
            info!("Despawning previous portal");
            if let Some(cam) = previous_portal.camera {
                commands.entity(cam).despawn_recursive();
            }
            commands.entity(entity).despawn_recursive();
        }
        let portal = PortalBundle::<N>::from_ray_impact(
            impact,
            &player_transform,
            &portal_res,
            other_portal_entity,
            rapier,
        );
        info!(
            "Spawning portal at {}",
            &portal.mesh_bundle.transform.translation
        );
        Some(commands.spawn_bundle(portal).id())
    }

    fn get_portal_plane(trf: &GlobalTransform) -> Vec4 {
        let normal = trf.back();
        let position = trf.translation() + (PORTAL_MESH_DEPTH) * normal;
        Vec4::from((normal.xyz(), -normal.dot(position)))
    }
}

#[derive(Debug, Default, Reflect)]
pub struct PortalResources {
    noise_texture: Handle<Image>,
    render_targets: [Handle<Image>; 2],
    open_materials: [Handle<OpenPortalMaterial>; 2],
    closed_materials: [Handle<ClosedPortalMaterial>; 2],
    portal_mesh: Handle<Mesh>,
    main_camera: Option<Entity>,
    dbg_sphere_mesh: Handle<Mesh>,
    dbg_material: Handle<StandardMaterial>,
}

#[derive(Debug, Default, Clone, Reflect)]
/// Enumerates the different cases for portal orientation that we handle differently.
pub enum PortalOrientation {
    /// The portal is horizontal on the ground or ceiling.
    Horizontal,
    /// The portal is on a surface which is neither the ground nor ceiling.
    #[default]
    Other,
}

#[derive(Debug, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Portal<const N: u32> {
    /// The camera which is used to render to the texture applied to this portal
    /// This camera is positioned to look at the other portal from behind, with the same relative
    /// position.
    camera: Option<Entity>,
    linked_portal: Option<Entity>,
    orientation: PortalOrientation,
}

impl<const N: u32> Portal<N> {
    /// Return the mouse button associated to shooting this portal type.
    pub const fn mouse_button() -> MouseButton {
        match N {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            _ => panic!("No such portal"),
        }
    }

    /// Return the collision groups filter which turns off collisions with this portal's surface.
    pub fn filter_collisions(&self) -> u32 {
        match self.orientation {
            PortalOrientation::Horizontal => {
                PLAYER_GROUP | PROPS_GROUP | PORTAL_GROUP | WALLS_GROUP
            }
            PortalOrientation::Other => PLAYER_GROUP | PROPS_GROUP | PORTAL_GROUP | GROUND_GROUP,
        }
    }

    /// Return the collision groups filter which turns collisions with this portal's surface back on.
    pub fn restore_collisions(&self) -> u32 {
        ALL_GROUPS
    }
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
pub struct PortalCamera<const N: u32>;

#[derive(Debug, SystemLabel)]
pub enum PortalLabels {
    ShootPortals,
    UpdateMainCamera,
    CreateCameras,
    SyncCameras,
    TeleportEntities,
    AnimateCamera,
}

#[derive(Debug, Component, Clone, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct PortalTeleport;

#[derive(Debug, Component, Clone, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct AnimateRoll {
    end: Quat,
    start: Quat,
    duration: Duration,
    remaining: Duration,
}

impl AnimateRoll {
    pub fn new(start: Quat, rotation: Quat, duration: Duration) -> AnimateRoll {
        AnimateRoll {
            end: rotation * start,
            duration,
            remaining: duration,
            start,
        }
    }
}

#[derive(Bundle)]
pub struct PortalBundle<const N: u32> {
    #[bundle]
    mesh_bundle: MaterialMeshBundle<OpenPortalMaterial>,
    render_layers: RenderLayers,
    portal: Portal<N>,
    collider: Collider,
    active_events: ActiveEvents,
    sensor: Sensor,
    collision_groups: CollisionGroups,
}

impl<const N: u32> Default for PortalBundle<N> {
    fn default() -> Self {
        PortalBundle {
            render_layers: RenderLayers::layer(1),
            collider: Collider::cuboid(0.5, 0.5, PORTAL_MESH_DEPTH / 2.),
            sensor: Sensor,
            active_events: ActiveEvents::COLLISION_EVENTS,
            collision_groups: CollisionGroups::new(PORTAL_GROUP, PLAYER_GROUP | PROPS_GROUP),
            mesh_bundle: MaterialMeshBundle::default(),
            portal: Portal::default(),
        }
    }
}

impl<const N: u32> PortalBundle<N> {
    fn from_ray_impact(
        impact: RayIntersection,
        player_transform: &GlobalTransform,
        portal_res: &Res<PortalResources>,
        other_portal: Option<Entity>,
        rapier: &Res<RapierContext>,
    ) -> PortalBundle<N> {
        const Z_FIGHTING_OFFSET: f32 = 0.001;
        // We place the portal at the ray intersection point, plus a small offset
        // along the surface normal to prevent Z fighting.
        let portal_center = impact.point + impact.normal * Z_FIGHTING_OFFSET;

        // Orient along the surface normal: we rotate the portal by the rotation between the object
        // space normal and the world space impact normal.
        let mut transform = Transform {
            translation: portal_center,
            ..default()
        };
        let (up, orientation) = if impact.normal.abs().abs_diff_eq(Vec3::Y, 0.001) {
            // If the normal is close to vertical, align the up direction with the player forward
            // direction.
            let forward_to_normal = player_transform
                .forward()
                .project_onto_normalized(impact.normal);
            (
                (player_transform.forward() - forward_to_normal).normalize(),
                PortalOrientation::Horizontal,
            )
        } else {
            // If the normal is not vertical, we can figure out the portal "up" direction by
            // projecting the Y vector onto the portal plane and normalizing the result.
            let y_to_normal = Vec3::Y.project_onto_normalized(impact.normal);
            (
                (Vec3::Y - y_to_normal).normalize(),
                PortalOrientation::Other,
            )
        };
        transform.translation =
            geometry::adjust_portal_origin_to_obstacles(portal_center, impact.normal, up, rapier);
        transform.look_at(transform.translation - impact.normal, up);

        // Offset the portal so the clipping plane coincides with the surface.
        let mut offset_portal = transform.with_scale(Vec3::splat(2.));
        offset_portal.translation += offset_portal.forward() * PORTAL_MESH_DEPTH;
        PortalBundle {
            mesh_bundle: MaterialMeshBundle {
                mesh: portal_res.portal_mesh.clone(),
                material: portal_res.open_materials[N as usize].clone(),
                transform: offset_portal,
                ..default()
            },
            portal: Portal::<N> {
                linked_portal: other_portal,
                orientation,
                ..default()
            },
            ..default()
        }
    }
}

const PORTAL_MESH_DEPTH: f32 = 0.5;

/// Load the assets required to render the portals.
fn load_portal_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<OpenPortalMaterial>>,
    mut closed_materials: ResMut<Assets<ClosedPortalMaterial>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let portal_mesh = meshes.add(
        shape::Box {
            min_x: -0.5,
            max_x: 0.5,
            min_y: -0.5,
            max_y: 0.5,
            // TODO: link to main camera near plane distance
            min_z: -PORTAL_MESH_DEPTH / 2.,
            max_z: PORTAL_MESH_DEPTH / 2.,
        }
        .into(),
    );

    let mut fbm = Fbm::new();
    fbm.octaves = 3;
    fbm.frequency = 0.5;
    fbm.lacunarity = 2.;
    fbm.persistence = 0.6;
    let noise_map = PlaneMapBuilder::new(&fbm)
        .set_size(1024, 1024)
        .set_x_bounds(-8.0, 8.0)
        .set_y_bounds(-8.0, 8.0)
        .build();
    let mut buf = Vec::with_capacity(1024 * 1024);
    for x in 0..1024 {
        for y in 0..1024 {
            buf.push((noise_map.get_value(x, y) * 255.) as u8);
        }
    }
    let noise_image = Image {
        data: buf,
        texture_descriptor: TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        },
        ..default()
    };
    //::new(, TextureDimension::D2, buf, TextureFormat::R8Unorm);
    let noise_texture = images.add(noise_image);

    let mut open_materials: [Handle<OpenPortalMaterial>; 2] = default();
    let mut closed_mats: [Handle<ClosedPortalMaterial>; 2] = default();
    closed_mats[0] = closed_materials.add(ClosedPortalMaterial {
        texture: noise_texture.clone(),
        // Orange
        color: Color::rgb_linear(1., 0.7, 0.2),
        time: 0.,
    });
    closed_mats[1] = closed_materials.add(ClosedPortalMaterial {
        texture: noise_texture.clone(),
        // Blue
        color: Color::rgb(0.2, 0.78, 1.),
        time: 0.,
    });

    let mut render_targets: [Handle<Image>; 2] = default();
    for i in 0..2 {
        let tex_size = Extent3d {
            width: 1280,
            height: 720,
            ..default()
        };
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size: tex_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };
        image.resize(tex_size);
        render_targets[i] = images.add(image);

        open_materials[i] = materials.add(OpenPortalMaterial {
            texture: render_targets[i].clone(),
        });
    }

    let dbg_mesh = meshes.add(
        shape::UVSphere {
            radius: 0.5,
            sectors: 12,
            stacks: 12,
        }
        .into(),
    );
    let dbg_mat = std_materials.add(Color::PURPLE.into());

    commands.insert_resource(PortalResources {
        render_targets,
        open_materials,
        closed_materials: closed_mats,
        portal_mesh,
        main_camera: None,
        dbg_sphere_mesh: dbg_mesh,
        dbg_material: dbg_mat,
        noise_texture,
    });
}

/// Obtain the main camera if not already present in the resources, or if it has been modified.
fn update_main_camera(
    mut commands: Commands,
    cameras_query: Query<(&mut Camera, Entity), With<FirstPersonCamera>>,
    windows: Res<Windows>,
    mut portal_res: ResMut<PortalResources>,
) {
    if portal_res.main_camera.is_none() && windows.get_primary().is_some() {
        let primary_win = windows.get_primary().unwrap();
        if let Ok((camera, entity)) = cameras_query.get_single() {
            if camera.target == RenderTarget::Window(primary_win.id()) {
                // Add the portals render layer so the main camera can render them.
                commands
                    .entity(entity)
                    .insert(RenderLayers::default().with(1));
                info!("Updating main camera to entity {:?}", entity);
                portal_res.main_camera = Some(entity);
            }
        }
    }
}

/// On left click/right click, shoot a portal.
fn fire_portal<const N: u32, const OTHER: u32>(
    mut commands: Commands,
    player_query: Query<&GlobalTransform, With<FirstPersonCamera>>,
    portal_query: Query<(&Portal<N>, Entity)>,
    other_portal_query: Query<Entity, With<Portal<OTHER>>>,
    rapier: Res<RapierContext>,
    mouse_buttons: Res<Input<MouseButton>>,
    portal_res: Res<PortalResources>,
) {
    if let Ok(player_pos) = player_query.get_single() {
        if mouse_buttons.just_pressed(Portal::<N>::mouse_button()) {
            info!("Shooting portal {}", N);
            PortalPlugin::spawn_portal(
                &mut commands,
                player_pos,
                &portal_query,
                other_portal_query.get_single().ok(),
                &rapier,
                &portal_res,
            );
        }
    }
}

fn create_portal_cameras<const N: u32>(
    mut commands: Commands,
    mut portal_query: Query<&mut Portal<N>>,
    portal_res: Res<PortalResources>,
) {
    if let Ok(mut portal) = portal_query.get_single_mut() {
        if portal.camera.is_none() && portal_res.main_camera.is_some() {
            portal.camera = Some(
                commands
                    .spawn_bundle(Camera3dBundle {
                        camera: Camera {
                            // Render before the main camera.
                            priority: -1 - N as isize,
                            target: RenderTarget::Image(
                                portal_res.render_targets[N as usize].clone(),
                            ),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(PortalCameraProjection {
                        fov: FRAC_PI_4,
                        aspect_ratio: 16. / 9.,
                        ..default()
                    })
                    .insert(PortalCamera::<N>)
                    .remove::<Projection>()
                    .insert_bundle(VisibilityBundle {
                        visibility: Visibility::visible(),
                        ..default()
                    })
                    .with_children(|camera| {
                        camera.spawn_bundle(PbrBundle {
                            mesh: portal_res.dbg_sphere_mesh.clone(),
                            material: portal_res.dbg_material.clone(),
                            ..default()
                        });
                    })
                    .id(),
            );
        }
    }
}

fn set_portal_materials(
    mut commands: Commands,
    portal_a_query: Query<Entity, (With<Portal<0>>, Without<Portal<1>>)>,
    portal_b_query: Query<Entity, (With<Portal<1>>, Without<Portal<0>>)>,
    resources: Res<PortalResources>,
) {
    match (portal_a_query.get_single(), portal_b_query.get_single()) {
        (Ok(portal_a), Ok(portal_b)) => {
            commands
                .entity(portal_a)
                .remove::<Handle<ClosedPortalMaterial>>()
                .insert(resources.open_materials[0].clone());
            commands
                .entity(portal_b)
                .remove::<Handle<ClosedPortalMaterial>>()
                .insert(resources.open_materials[1].clone());
        }
        (Ok(portal_a), Err(_)) => {
            commands
                .entity(portal_a)
                .remove::<Handle<OpenPortalMaterial>>()
                .insert(resources.closed_materials[0].clone());
        }
        (Err(_), Ok(portal_b)) => {
            commands
                .entity(portal_b)
                .remove::<Handle<OpenPortalMaterial>>()
                .insert(resources.closed_materials[1].clone());
        }
        (Err(_), Err(_)) => {}
    };
}

fn sync_portal_cameras(
    main_camera_query: Query<
        &GlobalTransform,
        (
            With<FirstPersonCamera>,
            Without<PortalCamera<0>>,
            Without<PortalCamera<1>>,
        ),
    >,
    portal_query_a: Query<
        &GlobalTransform,
        (
            With<Portal<0>>,
            Without<PortalCamera<0>>,
            Without<PortalCamera<1>>,
        ),
    >,
    portal_query_b: Query<
        &GlobalTransform,
        (
            With<Portal<1>>,
            Without<PortalCamera<0>>,
            Without<PortalCamera<1>>,
        ),
    >,
    mut portal_cam_a_query: Query<
        (&mut Transform, &mut PortalCameraProjection),
        (With<PortalCamera<0>>, Without<PortalCamera<1>>),
    >,
    mut portal_cam_b_query: Query<
        (&mut Transform, &mut PortalCameraProjection),
        (With<PortalCamera<1>>, Without<PortalCamera<0>>),
    >,
    mut lines: ResMut<DebugLines>,
) {
    if let (
        Ok(trf_a),
        Ok(trf_b),
        Ok(trf_main_cam),
        Ok((mut cam_a_trf, mut proj_a)),
        Ok((mut cam_b_trf, mut proj_b)),
    ) = (
        portal_query_a.get_single(),
        portal_query_b.get_single(),
        main_camera_query.get_single(),
        portal_cam_a_query.get_single_mut(),
        portal_cam_b_query.get_single_mut(),
    ) {
        let trf_main_cam = trf_main_cam.compute_transform();
        let ta = trf_a.compute_transform();
        let tb = trf_b.compute_transform();
        *cam_a_trf = geometry::portal_to_portal(&ta, &tb) * trf_main_cam;
        *cam_b_trf = geometry::portal_to_portal(&tb, &ta) * trf_main_cam;

        // Compute the clipping planes for both cameras.
        // The plane normals are the rotated forward() direction of the portal transforms, and their origin
        // is on the plane, which is enough to compute the plane homogeneous coords. They must be
        // transformed to the camera reference frame afterwards.
        let cam_a_clip_plane = PortalPlugin::get_portal_plane(trf_b);
        let cam_b_clip_plane = PortalPlugin::get_portal_plane(trf_a);

        // Inverse transpose of the view matrix = inverse inverse transpose of camera matrix = transpose
        proj_a.near = cam_a_trf.compute_matrix().transpose() * cam_a_clip_plane;
        proj_b.near = cam_b_trf.compute_matrix().transpose() * cam_b_clip_plane;
        let d = proj_a.near.xyz().length_recip();
        proj_a.near *= d;
        let d = proj_b.near.xyz().length_recip();
        proj_b.near *= d;

        #[cfg(feature = "devel")]
        {
            super::debug::draw::draw_camera_frustum_infinite_reverse(
                &cam_a_trf, &proj_a, &mut lines,
            );
            super::debug::draw::draw_camera_frustum_infinite_reverse(
                &cam_b_trf, &proj_b, &mut lines,
            );
        }
    }
}

fn turn_off_collisions_with_static_geo_when_in_portal(
    mut collisions: EventReader<CollisionEvent>,
    portal_a_query: Query<(Entity, &Portal<0>), Without<PortalTeleport>>,
    portal_b_query: Query<(Entity, &Portal<1>), Without<PortalTeleport>>,
    mut teleportable_query: Query<&mut CollisionGroups, With<PortalTeleport>>,
) {
    if let (Ok((portal_a_entity, portal_a)), Ok((portal_b_entity, portal_b))) =
        (portal_a_query.get_single(), portal_b_query.get_single())
    {
        for collision in collisions.iter() {
            match collision {
                CollisionEvent::Started(collider_a, collider_b, _flags) => {
                    if collider_a == &portal_a_entity || collider_b == &portal_a_entity {
                        let maybe_teleportable = if collider_a == &portal_a_entity {
                            collider_b
                        } else {
                            collider_a
                        };
                        if let Ok(mut groups) = teleportable_query.get_mut(*maybe_teleportable) {
                            groups.filters = portal_a.filter_collisions();
                        }
                    } else if collider_a == &portal_b_entity || collider_b == &portal_b_entity {
                        let maybe_teleportable = if collider_a == &portal_b_entity {
                            collider_b
                        } else {
                            collider_a
                        };
                        if let Ok(mut groups) = teleportable_query.get_mut(*maybe_teleportable) {
                            groups.filters = portal_b.filter_collisions();
                        }
                    }
                }
                CollisionEvent::Stopped(collider_a, collider_b, _flags) => {
                    if collider_a == &portal_a_entity || collider_b == &portal_a_entity {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_b) {
                            groups.filters = portal_a.restore_collisions();
                        }
                    } else if collider_a == &portal_b_entity || collider_b == &portal_b_entity {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_a) {
                            groups.filters = portal_b.restore_collisions();
                        }
                    }
                }
            }
        }
    }
}

fn teleport_props(
    portal_a_query: Query<(&Transform, Entity), (With<Portal<0>>, Without<PortalTeleport>)>,
    portal_b_query: Query<(&Transform, Entity), (With<Portal<1>>, Without<PortalTeleport>)>,
    mut teleportables: Query<
        (&mut Transform, &mut Velocity),
        (With<PortalTeleport>, Without<FirstPersonController>),
    >,
) {
    const PROXIMITY_THRESHOLD: f32 = 1.0;
    if let (Ok((portal_a_trf, _portal_a)), Ok((portal_b_trf, _portal_b))) =
        (portal_a_query.get_single(), portal_b_query.get_single())
    {
        let mut a_to_b = None;
        let mut b_to_a = None;
        for (mut obj_transform, mut velocity) in &mut teleportables {
            let a_clip_to_object = obj_transform.translation - portal_a_trf.translation
                + portal_a_trf.forward() * PORTAL_MESH_DEPTH;
            let b_clip_to_object = obj_transform.translation - portal_b_trf.translation
                + portal_b_trf.forward() * PORTAL_MESH_DEPTH;
            if a_clip_to_object.length() < PROXIMITY_THRESHOLD {
                if a_clip_to_object.dot(portal_a_trf.forward()) > 0. {
                    info!("Teleporting object from portal A to portal B");
                    let transform = a_to_b.get_or_insert_with(|| {
                        geometry::portal_to_portal(portal_a_trf, portal_b_trf)
                    });
                    *obj_transform = transform.mul_transform(*obj_transform);
                    velocity.linvel = transform.rotation.mul_vec3(velocity.linvel);
                    velocity.angvel = transform.rotation.mul_vec3(velocity.angvel);
                }
            } else if b_clip_to_object.length() < PROXIMITY_THRESHOLD
                && b_clip_to_object.dot(portal_b_trf.forward()) > 0.
            {
                info!("Teleporting object from portal B to portal A");
                let transform = b_to_a
                    .get_or_insert_with(|| geometry::portal_to_portal(portal_b_trf, portal_a_trf));
                *obj_transform = transform.mul_transform(*obj_transform);
                velocity.linvel = transform.rotation.mul_vec3(velocity.linvel);
                velocity.angvel = transform.rotation.mul_vec3(velocity.angvel);
            }
        }
    }
}

// Player teleportation is handled differently from objects :
// * We keep the player capsule collider vertical at all times
// * We transform the player position normally, but the camera orientation requires some
//   special care. If the computed transform is does not keep the player upright, then
//   we introduce a short animation bringing the camera back in line with the physical model.
fn teleport_player(
    mut commands: Commands,
    portal_a_query: Query<(&Transform, Entity), (With<Portal<0>>, Without<PortalTeleport>)>,
    portal_b_query: Query<(&Transform, Entity), (With<Portal<1>>, Without<PortalTeleport>)>,
    mut player: Query<
        (
            &mut Transform,
            &mut Velocity,
            &mut FirstPersonController,
            Entity,
        ),
        With<PortalTeleport>,
    >,
    mut camera_query: Query<
        (&mut Transform, &GlobalTransform),
        (
            With<CameraAnchor>,
            Without<Portal<0>>,
            Without<Portal<1>>,
            Without<PortalTeleport>,
        ),
    >,
    rapier: Res<RapierContext>,
) {
    // Player origin is on the ground, so offset the detection distance a bit
    const PLAYER_PROXIMITY_THRESHOLD: f32 = 2.3;
    const MIN_OUTBOUND_SPEED: f32 = 3.;
    if let (Ok((portal_a_trf, _portal_a)), Ok((portal_b_trf, _portal_b))) =
        (portal_a_query.get_single(), portal_b_query.get_single())
    {
        if let (
            Ok((mut player_transform, mut velocity, mut player_controller, player_entity)),
            Ok((mut camera_transform, camera_global)),
        ) = (player.get_single_mut(), camera_query.get_single_mut())
        {
            let a_clip_to_player = player_transform.translation - portal_a_trf.translation
                + portal_a_trf.forward() * PORTAL_MESH_DEPTH;
            let b_clip_to_player = player_transform.translation - portal_b_trf.translation
                + portal_b_trf.forward() * PORTAL_MESH_DEPTH;
            if a_clip_to_player.length() < PLAYER_PROXIMITY_THRESHOLD {
                if a_clip_to_player.dot(portal_a_trf.forward()) > 0. {
                    info!("Teleporting player from portal A to portal B");
                    let a_to_b = geometry::portal_to_portal(&portal_a_trf, &portal_b_trf);
                    geometry::adjust_player_camera_on_teleport(
                        &a_to_b,
                        &camera_global.compute_transform(),
                        &mut camera_transform,
                        player_entity,
                        &mut player_transform,
                        &mut player_controller,
                    );

                    let output_direction = portal_b_trf.back();
                    let transformed_velocity = a_to_b.rotation.mul_vec3(velocity.linvel);
                    velocity.linvel = portal_b_trf.back() * transformed_velocity.length();
                    if velocity.linvel.dot(output_direction) < MIN_OUTBOUND_SPEED {
                        velocity.linvel += MIN_OUTBOUND_SPEED * output_direction;
                    }
                }
            } else if b_clip_to_player.length() < PLAYER_PROXIMITY_THRESHOLD {
                if b_clip_to_player.dot(portal_b_trf.forward()) > 0. {
                    info!("Teleporting player from portal B to portal A");
                    let b_to_a = geometry::portal_to_portal(&portal_b_trf, &portal_a_trf);
                    geometry::adjust_player_camera_on_teleport(
                        &b_to_a,
                        &camera_global.compute_transform(),
                        &mut camera_transform,
                        player_entity,
                        &mut player_transform,
                        &mut player_controller,
                    );

                    let output_direction = portal_a_trf.back();
                    let transformed_velocity = b_to_a.rotation.mul_vec3(velocity.linvel);
                    velocity.linvel = portal_a_trf.back() * transformed_velocity.length();
                    if velocity.linvel.dot(output_direction) < MIN_OUTBOUND_SPEED {
                        velocity.linvel += MIN_OUTBOUND_SPEED * output_direction;
                    }
                }
            }
        }
    }
}

fn animate_camera_roll(
    mut commands: Commands,
    mut player_query: Query<
        (&mut Transform, &mut AnimateRoll, Entity),
        With<FirstPersonController>,
    >,
    time: Res<Time>,
) {
    for (mut transform, mut animation, entity) in &mut player_query {
        if time.delta() > animation.remaining {
            // Apply the full remaining transformation
            transform.rotation = animation.end;
            commands
                .entity(entity)
                .remove::<AnimateRoll>()
                .remove::<CameraLock>();
            info!("Roll animation completed");
        } else {
            let elapsed_total = animation.duration - animation.remaining + time.delta();
            let s = elapsed_total.as_secs_f32() / animation.duration.as_secs_f32();
            transform.rotation = animation.start.slerp(animation.end, s);
            animation.remaining -= time.delta();
        }
    }
}
