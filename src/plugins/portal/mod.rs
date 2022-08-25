use std::f32::consts::{FRAC_PI_4, PI};

use bevy::{
    math::Vec4Swizzles,
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
use bevy_rapier3d::prelude::*;

mod camera_projection;
mod material;

use camera_projection::PortalCameraProjection;
use material::PortalMaterial;

use super::{first_person_controller::*, physics::*};

#[derive(Debug)]
pub struct PortalPlugin;

// TODO:
//
// * Transition between the open and closed materials depending on whether there are 1 or 2 portals open
// * Figure where to place the portal cameras
//   * Same thing for recursive portal iterations

#[derive(Debug, Default, Reflect)]
struct PortalResources {
    texture_a: Handle<Image>,
    texture_b: Handle<Image>,
    render_targets: [Handle<Image>; 2],
    open_materials: [Handle<PortalMaterial>; 2],
    mesh: Handle<Mesh>,
    main_camera: Option<Entity>,
    dbg_sphere_mesh: Handle<Mesh>,
    dbg_material: Handle<StandardMaterial>,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
pub struct Portal<const N: u32> {
    /// The camera which is used to render to the texture applied to this portal
    /// This camera is positioned to look at the other portal from behind, with the same relative
    /// position.
    camera: Option<Entity>,
    linked_portal: Option<Entity>,
}

#[derive(Debug, Default, Component, Reflect, FromReflect)]
pub struct PortalCamera<const N: u32>;

impl<const N: u32> Portal<N> {
    pub const fn mouse_button() -> MouseButton {
        match N {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            _ => panic!("No such portal"),
        }
    }
}

#[derive(Debug, SystemLabel)]
pub enum PortalLabels {
    ShootPortals,
    UpdateMainCamera,
    CreateCameras,
    SyncCameras,
    TeleportEntities,
}

#[derive(Debug, Component, Clone, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct PortalTeleport;

#[derive(Bundle)]
pub struct PortalBundle<const N: u32> {
    #[bundle]
    mesh_bundle: MaterialMeshBundle<PortalMaterial>,
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
            collider: Collider::cuboid(1., 1., 0.6),
            sensor: Sensor,
            active_events: ActiveEvents::COLLISION_EVENTS,
            collision_groups: CollisionGroups::new(PORTAL_GROUP, PLAYER_GROUP | PROPS_GROUP),
            mesh_bundle: MaterialMeshBundle::default(),
            portal: Portal::default(),
        }
    }
}

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<PortalMaterial>::default())
            .register_type::<Portal<0>>()
            .register_type::<Portal<1>>()
            .register_type::<PortalResources>()
            .register_type::<PortalMaterial>()
            .register_type::<PortalTeleport>()
            .add_plugin(bevy::render::camera::CameraProjectionPlugin::<
                PortalCameraProjection,
            >::default())
            .add_startup_system(load_portal_assets)
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
                    //.with_system(teleport_player)
                    .label(PortalLabels::TeleportEntities)
                    .after(PortalLabels::SyncCameras),
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
        player_pos: &GlobalTransform,
        portal_query: &Query<(&Portal<N>, Entity)>,
        other_portal_entity: Option<Entity>,
        rapier: &Res<RapierContext>,
        portal_res: &Res<PortalResources>,
    ) -> Option<Entity> {
        if let Some(portal_pos) = PortalPlugin::portal_location(player_pos, rapier) {
            info!("Spawning portal at {}", portal_pos.translation);
            if let Ok((previous_portal, entity)) = portal_query.get_single() {
                info!("Despawning previous portal");
                if let Some(cam) = previous_portal.camera {
                    commands.entity(cam).despawn_recursive();
                }
                commands.entity(entity).despawn_recursive();
            }
            Some(
                commands
                    .spawn_bundle(PortalBundle {
                        mesh_bundle: MaterialMeshBundle {
                            mesh: portal_res.mesh.clone(),
                            material: portal_res.open_materials[N as usize].clone(),
                            transform: portal_pos,
                            ..default()
                        },
                        portal: Portal::<N> {
                            linked_portal: other_portal_entity,
                            ..default()
                        },
                        ..default()
                    })
                    .id(),
            )
        } else {
            None
        }
    }

    fn portal_location(
        player_transform: &GlobalTransform,
        rapier: &Res<RapierContext>,
    ) -> Option<Transform> {
        let (_entity, intersection) = rapier.cast_ray_and_get_normal(
            player_transform.translation(),
            player_transform.forward(),
            Real::MAX,
            true,
            QueryFilter::only_fixed(),
        )?;
        Some(Self::location_from_impact(intersection))
    }

    fn location_from_impact(impact: RayIntersection) -> Transform {
        const Z_FIGHTING_OFFSET: f32 = 0.001;

        let mut transform = Transform {
            // We place the portal at the ray intersection point, plus a small offset
            // along the surface normal to prevent Z fighting.
            translation: impact.point + impact.normal * Z_FIGHTING_OFFSET,
            ..default()
        };
        // Orient along the surface normal
        // TODO: we assume a vertical portal position for now, try and figure out
        // the math for correctly placing the portal later.
        transform.look_at(impact.point - impact.normal, Vec3::Y);
        transform
    }
}

/// Load the assets required to render the portals.
fn load_portal_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let tex_a = assets.load("textures/portal_a.png");
    let tex_b = assets.load("textures/portal_b.png");
    let portal_mesh = meshes.add(
        shape::Quad {
            size: Vec2::new(2., 2.),
            flip: false,
        }
        .into(),
    );
    let mut open_materials: [Handle<PortalMaterial>; 2] = default();

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

        open_materials[i] = materials.add(PortalMaterial {
            texture: render_targets[i].clone(),
        })
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
        texture_a: tex_a,
        texture_b: tex_b,
        render_targets,
        open_materials,
        mesh: portal_mesh,
        main_camera: None,
        dbg_sphere_mesh: dbg_mesh,
        dbg_material: dbg_mat,
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
                other_portal_query
                    .get_single()
                    .ok(),
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
                    .id(),
            );
        }
    }
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
        &Transform,
        (
            With<Portal<0>>,
            Without<PortalCamera<0>>,
            Without<PortalCamera<1>>,
        ),
    >,
    portal_query_b: Query<
        &Transform,
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
        // Position the render camera for portal A behind portal B
        // For this, we express the transformation between the main camera and portal A, then
        // apply it between the virtual camera and portal B.
        let trf_main_cam = trf_main_cam.compute_transform();
        let rot = Transform::from_rotation(Quat::from_rotation_y(PI));
        *cam_a_trf = (*trf_b * rot * Transform::from_matrix(trf_a.compute_matrix().inverse()))
            * trf_main_cam;
        *cam_b_trf = (*trf_a * rot * Transform::from_matrix(trf_b.compute_matrix().inverse()))
            * trf_main_cam;

        // Compute the clipping planes for both cameras.
        // The plane normals are the rotated forward() direction of the portal transforms, and their origin
        // is on the plane, which is enough to compute the plane homogeneous coords. They must be
        // transformed to the camera reference frame afterwards.
        let portal_a_normal = rot.rotation.mul_vec3(trf_a.forward());
        let portal_b_normal = rot.rotation.mul_vec3(trf_b.forward());
        let cam_a_clip_plane =
            Vec4::from((portal_b_normal, -portal_b_normal.dot(trf_b.translation)));
        let cam_b_clip_plane =
            Vec4::from((portal_a_normal, -portal_a_normal.dot(trf_a.translation)));
        // Inverse transpose of the view matrix = inverse inverse transpose of camera matrix = transpose
        proj_a.near = cam_a_trf.compute_matrix().transpose() * cam_a_clip_plane;
        proj_b.near = cam_b_trf.compute_matrix().transpose() * cam_b_clip_plane;
        let d = proj_a.near.xyz().length_recip();
        proj_a.near *= d;
        let d = proj_b.near.xyz().length_recip();
        proj_b.near *= d;
    }
}

fn turn_off_collisions_with_static_geo_when_in_portal(
    mut collisions: EventReader<CollisionEvent>,
    portal_a_query: Query<Entity, (With<Portal<0>>, Without<PortalTeleport>)>,
    portal_b_query: Query<Entity, (With<Portal<1>>, Without<PortalTeleport>)>,
    mut teleportable_query: Query<&mut CollisionGroups, With<PortalTeleport>>,
) {
    if let (Ok(portal_a), Ok(portal_b)) = (portal_a_query.get_single(), portal_b_query.get_single())
    {
        for collision in collisions.iter() {
            match collision {
                CollisionEvent::Started(collider_a, collider_b, _flags) => {
                    info!(
                        "Collision started between {:?} and {:?}",
                        collider_a, collider_b
                    );
                    if collider_a == &portal_a || collider_a == &portal_b {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_b) {
                            info!("Teleportable object in portal sensor zone");
                            groups.filters = PLAYER_GROUP | PROPS_GROUP | PORTAL_GROUP;
                        }
                    } else if collider_b == &portal_a || collider_b == &portal_b {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_a) {
                            info!("Teleportable object in portal sensor zone");
                            groups.filters = PLAYER_GROUP | PROPS_GROUP | PORTAL_GROUP;
                        }
                    }
                }
                CollisionEvent::Stopped(collider_a, collider_b, _flags) => {
                    info!(
                        "Collision stopped between {:?} and {:?}",
                        collider_a, collider_b
                    );
                    if collider_a == &portal_a || collider_a == &portal_b {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_b) {
                            info!("Teleportable object out of portal sensor zone");
                            groups.filters = ALL_GROUPS;
                        }
                    } else if collider_b == &portal_a || collider_b == &portal_b {
                        if let Ok(mut groups) = teleportable_query.get_mut(*collider_a) {
                            info!("Teleportable object out of portal sensor zone");
                            groups.filters = ALL_GROUPS;
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
    mut teleportables: Query<(&mut Transform, &mut Velocity), With<PortalTeleport>>,
) {
    const PROXIMITY_THRESHOLD: f32 = 1.0;
    if let (Ok((portal_a_trf, _portal_a)), Ok((portal_b_trf, _portal_b))) =
        (portal_a_query.get_single(), portal_b_query.get_single())
    {
        let flip = Mat4::from_rotation_y(PI);
        let a_to_b = Transform::from_matrix(
            portal_b_trf.compute_matrix() * flip * portal_a_trf.compute_matrix().inverse(),
        );
        let b_to_a = Transform::from_matrix(
            portal_a_trf.compute_matrix() * flip * portal_b_trf.compute_matrix().inverse(),
        );
        for (mut obj_transform, mut velocity) in &mut teleportables {
            let a_to_object = obj_transform.translation - portal_a_trf.translation;
            let b_to_object = obj_transform.translation - portal_b_trf.translation;
            if a_to_object.length() < PROXIMITY_THRESHOLD {
                if a_to_object.dot(portal_a_trf.forward()) > 0. {
                    info!("Teleporting object from portal A to portal B");
                    *obj_transform = a_to_b.mul_transform(*obj_transform);
                    velocity.linvel = a_to_b.rotation.mul_vec3(velocity.linvel);
                    velocity.angvel = a_to_b.rotation.mul_vec3(velocity.angvel);
                }
            } else if b_to_object.length() < PROXIMITY_THRESHOLD
                && b_to_object.dot(portal_b_trf.forward()) > 0.
            {
                info!("Teleporting object from portal B to portal A");
                *obj_transform = b_to_a.mul_transform(*obj_transform);
                velocity.linvel = b_to_a.rotation.mul_vec3(velocity.linvel);
                velocity.angvel = b_to_a.rotation.mul_vec3(velocity.angvel);
            }
        }
    }
}

//fn teleport_player(
//portal_a_query: Query<(&Transform, Entity), (With<Portal<0>>, Without<PortalTeleport>)>,
//portal_b_query: Query<(&Transform, Entity), (With<Portal<1>>, Without<PortalTeleport>)>,
//mut player: Query<(&mut Transform, &mut Velocity), (With<FirstPersonController>, With<PortalTeleport>)>,
//) {
//// Player origin is on the ground, so offset the detection distance a bit
//const PLAYER_PROXIMITY_THRESHOLD: f32 = 2.3;
//if let (Ok((portal_a_trf, _portal_a)), Ok((portal_b_trf, _portal_b))) =
//(portal_a_query.get_single(), portal_b_query.get_single())
//{
//let flip = Mat4::from_rotation_y(PI);
//let a_to_b = Transform::from_matrix(portal_b_trf.compute_matrix() * flip * portal_a_trf.compute_matrix().inverse());
//let b_to_a = Transform::from_matrix(portal_a_trf.compute_matrix() * flip * portal_b_trf.compute_matrix().inverse());
//if let (Ok((mut player_transform, mut velocity)), Ok(mut render_transform)) = (logical_player.get_single_mut(), render_player.get_single_mut()) {
//let a_to_player = player_transform.translation - portal_a_trf.translation;
//let b_to_player = player_transform.translation - portal_b_trf.translation;
//if a_to_player.length() < PLAYER_PROXIMITY_THRESHOLD {
//if a_to_player.dot(portal_a_trf.forward()) > 0. {
//info!("Teleporting player from portal A to portal B");
//*player_transform = a_to_b.mul_transform(*player_transform);
//*render_transform = a_to_b.mul_transform(*render_transform);
//velocity.linvel = a_to_b.rotation.mul_vec3(velocity.linvel);
//velocity.angvel = a_to_b.rotation.mul_vec3(velocity.angvel);
//}
//}
//else if b_to_player.length() < PLAYER_PROXIMITY_THRESHOLD {
//if b_to_player.dot(portal_b_trf.forward()) > 0. {
//info!("Teleporting player from portal B to portal A");
//*player_transform = b_to_a.mul_transform(*player_transform);
//*render_transform = b_to_a.mul_transform(*render_transform);
//velocity.linvel = b_to_a.rotation.mul_vec3(velocity.linvel);
//velocity.angvel = b_to_a.rotation.mul_vec3(velocity.angvel);
//}
//}
//}
//}
//}
