use bevy::{render::camera::CameraProjection, prelude::*, math::Vec4Swizzles};
use bevy_prototype_debug_lines::DebugLines;

pub fn draw_camera_frustum<P, R>(cam_transform: &Transform, projection: R, lines: &mut ResMut<DebugLines>) 
    where P: CameraProjection,
          R: AsRef<P>
{
    const NEAR_COLOR: Color = Color::BLACK;
    const FAR_COLOR: Color = Color::WHITE;

    let inv_viewprojection = (projection.as_ref().get_projection_matrix() * cam_transform.compute_matrix().inverse()).inverse();

    let frustum_corners_world = [
        Vec4::new(-1., -1., 0., 1.),
        Vec4::new( 1., -1., 0., 1.),
        Vec4::new(-1.,  1., 0., 1.),
        Vec4::new( 1.,  1., 0., 1.),
        Vec4::new(-1., -1.,  1., 1.),
        Vec4::new( 1., -1.,  1., 1.),
        Vec4::new(-1.,  1.,  1., 1.),
        Vec4::new( 1.,  1.,  1., 1.),
    ].into_iter().map(|v| {
        let vh = inv_viewprojection * v;
        vh.xyz() / vh.w
    }).collect::<Vec<_>>();

    // Depth lines
    lines.line_gradient(frustum_corners_world[0], frustum_corners_world[4], 0., NEAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[1], frustum_corners_world[5], 0., NEAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[2], frustum_corners_world[6], 0., NEAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[3], frustum_corners_world[7], 0., NEAR_COLOR, FAR_COLOR);

    // Near plane
    lines.line_gradient(frustum_corners_world[0], frustum_corners_world[1], 0., NEAR_COLOR, NEAR_COLOR);
    lines.line_gradient(frustum_corners_world[0], frustum_corners_world[2], 0., NEAR_COLOR, NEAR_COLOR);
    lines.line_gradient(frustum_corners_world[1], frustum_corners_world[3], 0., NEAR_COLOR, NEAR_COLOR);
    lines.line_gradient(frustum_corners_world[2], frustum_corners_world[3], 0., NEAR_COLOR, NEAR_COLOR);

    // Far plane
    lines.line_gradient(frustum_corners_world[4], frustum_corners_world[5], 0., FAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[4], frustum_corners_world[6], 0., FAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[5], frustum_corners_world[7], 0., FAR_COLOR, FAR_COLOR);
    lines.line_gradient(frustum_corners_world[6], frustum_corners_world[7], 0., FAR_COLOR, FAR_COLOR);
}
