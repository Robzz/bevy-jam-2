use std::f32::consts::FRAC_PI_4;

use bevy::{
    prelude::*,
    render::camera::{CameraProjection, DepthCalculation},
};

/// Camera projection which allows for an oblique near clipping plane, used for rendering portal virtual cameras.
#[derive(Debug, Component, Clone, Reflect)]
#[reflect(Component)]
pub struct PortalCameraProjection {
    pub fov: f32,
    pub aspect_ratio: f32,
    pub far: f32,
    #[reflect(ignore)]
    pub near: Vec4,
}

impl Default for PortalCameraProjection {
    fn default() -> Self {
        PortalCameraProjection {
            fov: FRAC_PI_4,
            aspect_ratio: 16. / 9.,
            far: 1000.,
            near: Vec4::from((Vec3::NEG_Z, -0.1)),
        }
    }
}

impl CameraProjection for PortalCameraProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        // Math taken from https://www.terathon.com/lengyel/Lengyel-Oblique.pdf
        let proj_mat = Mat4::perspective_infinite_rh(self.fov, self.aspect_ratio, 0.5);
        let proj_mat_inv = proj_mat.inverse();
        let mut oblique_proj_mat = proj_mat;

        let c = self.near;
        let m4 = proj_mat.row(3);
        let qp = Vec4::new(c.x.signum(), c.y.signum(), 1., 1.);
        let q = proj_mat_inv * qp;
        let a = 1. * m4.dot(q) / c.dot(q);
        let new_m3 = a * c;
        oblique_proj_mat.x_axis.z = new_m3.x;
        oblique_proj_mat.y_axis.z = new_m3.y;
        oblique_proj_mat.z_axis.z = new_m3.z;
        oblique_proj_mat.w_axis.z = new_m3.w;

        oblique_proj_mat
    }

    fn update(&mut self, width: f32, height: f32) {
        self.aspect_ratio = width / height;
    }

    fn depth_calculation(&self) -> DepthCalculation {
        DepthCalculation::Distance
    }

    fn far(&self) -> f32 {
        self.far
    }
}
