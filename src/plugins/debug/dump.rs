use std::{path::Path, borrow::Borrow};

use bevy::{prelude::{Image as BevyImage, error}, render::render_resource::TextureFormat};
use image::RgbaImage;

pub fn dump_texture<I: Borrow<BevyImage>, P: AsRef<Path>>(img: I, out_path: P) {
    let img = img.borrow();
    if img.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb || img.texture_descriptor.format == TextureFormat::Bgra8UnormSrgb {
        let size = img.size();
        let img = RgbaImage::from_raw(size.x as u32, size.y as u32, img.data.chunks_exact(4).map(|p| [p[2], p[1], p[0], p[3]]).flatten().collect::<Vec<_>>()).unwrap();
        img.save(out_path).unwrap();
    }
    else {
        error!("Unexpected texture format: {:?}", img.texture_descriptor.format);
    }
}
