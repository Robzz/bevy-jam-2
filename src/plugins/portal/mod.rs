use std::f32::consts::{FRAC_PI_4, PI};

use bevy::{
    prelude::*,
    reflect::FromReflect,
    render::{
        camera::{Projection, RenderTarget},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
    transform::TransformSystem, math::Vec4Swizzles,
};
use bevy_fps_controller::controller::RenderPlayer;
use bevy_rapier3d::prelude::*;

mod camera_projection;
mod material;

use camera_projection::PortalCameraProjection;
use material::PortalMaterial;

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
}

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<PortalMaterial>::default())
            .register_type::<Portal<0>>()
            .register_type::<Portal<1>>()
            .register_type::<PortalResources>()
            .register_type::<PortalMaterial>()
            .add_plugin(bevy::render::camera::CameraProjectionPlugin::<
                PortalCameraProjection,
            >::default())
            .add_startup_system(load_portal_assets)
            .add_system(update_main_camera.label(PortalLabels::UpdateMainCamera))
            .add_system_set(
                SystemSet::new()
                    .label(PortalLabels::ShootPortals)
                    .with_system(fire_portal::<0>)
                    .with_system(fire_portal::<1>),
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
        player_pos: &Transform,
        portal_query: &Query<(&Portal<N>, Entity)>,
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
                    .spawn_bundle(MaterialMeshBundle {
                        mesh: portal_res.mesh.clone(),
                        material: portal_res.open_materials[N as usize].clone(),
                        transform: portal_pos,
                        ..default()
                    })
                    // Render portals on a separate layer so the portal cameras can turn them off
                    .insert_bundle((
                        RenderLayers::layer(1),
                        Portal::<N>::default(),
                        Collider::cuboid(1., 1., 0.6),
                        Sensor
                    ))
                    .id(),
            )
        } else {
            None
        }
    }

    fn portal_location(
        player_transform: &Transform,
        rapier: &Res<RapierContext>,
    ) -> Option<Transform> {
        let (_entity, intersection) = rapier.cast_ray_and_get_normal(
            player_transform.translation,
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
    mut cameras_query: Query<(&mut Camera, Entity), (With<Camera3d>, With<RenderPlayer>)>,
    windows: Res<Windows>,
    mut portal_res: ResMut<PortalResources>,
) {
    if portal_res.main_camera.is_none() && windows.get_primary().is_some() {
        let primary_win = windows.get_primary().unwrap();
        if let Ok((camera, entity)) = cameras_query.get_single_mut() {
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
fn fire_portal<const N: u32>(
    mut commands: Commands,
    player_query: Query<&Transform, With<RenderPlayer>>,
    portal_query: Query<(&Portal<N>, Entity)>,
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
                            target: RenderTarget::Image(dbg!(portal_res.render_targets
                                [N as usize]
                                .clone())),
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
        &Transform,
        (
            With<RenderPlayer>,
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
        let rot = Transform::from_rotation(Quat::from_rotation_y(PI));
        *cam_a_trf = (*trf_b * rot * Transform::from_matrix(trf_a.compute_matrix().inverse()))
            * *trf_main_cam;
        *cam_b_trf = (*trf_a * rot * Transform::from_matrix(trf_b.compute_matrix().inverse()))
            * *trf_main_cam;

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
