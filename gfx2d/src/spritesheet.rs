use super::*;
use binpack::pack_rects;
use image::{self, GenericImage, RgbaImage as Image};
use std::path::PathBuf;

type Rect = binpack::Rect<(usize, usize)>;

#[derive(Debug, Clone)]
pub struct Sprite {
    pub width: f32,
    pub height: f32,
    pub texcoords_x: (f32, f32),
    pub texcoords_y: (f32, f32),
    pub texture: Option<Texture>,
}

#[derive(Debug)]
pub struct SpriteInfo {
    pub filename: PathBuf,
    pub pixel_ratio: Vec2,
    pub color_key: Option<Color>,
}

impl SpriteInfo {
    pub fn new(filename: PathBuf, pixel_ratio: Vec2, color_key: Option<Color>) -> SpriteInfo {
        SpriteInfo{filename, pixel_ratio, color_key}
    }
}

#[derive(Debug)]
pub struct Spritesheet {
    pub textures: Vec<Texture>,
    pub sprites: Vec<Sprite>,
}

impl Sprite {
    pub fn new(width: f32, height: f32, tx: (f32, f32), ty: (f32, f32), texture: Option<&Texture>) -> Sprite {
        Sprite {
            width,
            height,
            texcoords_x: tx,
            texcoords_y: ty,
            texture: texture.cloned(),
        }
    }

    pub fn from_texture(texture: &Texture, pixel_ratio: Vec2) -> Sprite {
        let (w, h) = texture.dimensions();

        Sprite {
            width: w as f32 / pixel_ratio.x,
            height: h as f32 / pixel_ratio.y,
            texcoords_x: (0.0, 1.0),
            texcoords_y: (0.0, 1.0),
            texture: Some(texture.clone()),
        }
    }
}

impl Spritesheet {
    pub fn empty() -> Spritesheet {
        Spritesheet{textures: vec![], sprites: vec![]}
    }

    pub fn new(context: &mut Gfx2dContext, padding: i32, filter: FilterMethod, info: &[SpriteInfo]) -> Spritesheet {
        if info.is_empty() {
            return Spritesheet::empty();
        }

        let max_size = context.max_texture_size() as i32;
        let mut images: Vec<Image> = Vec::with_capacity(info.len());
        let mut sprites: Vec<Sprite> = Vec::with_capacity(info.len());
        let mut rects: Vec<Rect> = Vec::with_capacity(info.len());

        for (index, ref sprite_info) in info.iter().enumerate() {
            let mut img = if sprite_info.filename.exists() {
                gfx2d_extra::load_image_rgba(&sprite_info.filename)
            } else {
                Image::from_pixel(1, 1, image::Rgba([0u8; 4]))
            };

            if let Some(color) = sprite_info.color_key {
                gfx2d_extra::remove_color_key(&mut img, color);
            }

            gfx2d_extra::premultiply_image(&mut img);

            sprites.push(Sprite {
                width: img.width() as f32 / sprite_info.pixel_ratio.x,
                height: img.height() as f32 / sprite_info.pixel_ratio.y,
                texcoords_x: (0.0, 0.0),
                texcoords_y: (0.0, 0.0),
                texture: None,
            });

            let mut w = i32::min(max_size, img.width() as i32);
            let mut h = i32::min(max_size, img.height() as i32);

            if w != img.width() as i32 || h != img.height() as i32 {
                img = image::imageops::resize(&img, w as u32, h as u32, image::FilterType::Lanczos3);
            }

            rects.push(binpack::Rect {
                x: 0,
                y: 0,
                w: img.width() as i32 + padding,
                h: img.height() as i32 + padding,
                data: (index, 0),
            });

            images.push(img);
        }

        let mut sheets: Vec<(i32, i32)> = Vec::new();
        Self::pack_recursive(&mut rects[..], &mut sheets, padding, max_size);

        let mut sheets: Vec<Image> = sheets.iter().map(|s| Image::new(s.0 as u32, s.0 as u32)).collect();

        for rc in &rects {
            let image_index = rc.data.0;
            let sheet_index = rc.data.1;
            sheets[sheet_index].copy_from(&images[image_index], rc.x as u32, rc.y as u32);
        }

        let textures: Vec<Texture> = sheets.drain(..).map(|ref img| {
            Texture::new(context, (img.width() as u16, img.height() as u16), img, filter, WrapMode::Clamp)
        }).collect();

        for rc in &rects {
            let sprite = &mut sprites[rc.data.0];
            let texture = &textures[rc.data.1];

            let (w, h) = texture.dimensions();
            let (x0, x1) = (rc.left() as f32, (rc.right() - padding) as f32);
            let (y0, y1) = (rc.top() as f32, (rc.bottom() - padding) as f32);

            sprite.texture = Some(texture.clone());
            sprite.texcoords_x = (x0 / w as f32, x1 / w as f32);
            sprite.texcoords_y = (y0 / h as f32, y1 / h as f32);
        }

        Spritesheet{textures, sprites}
    }

    fn pack_recursive(rects: &mut [Rect], sheets: &mut Vec<(i32, i32)>, pad: i32, max_size: i32) {
        if rects.len() == 1 {
            rects[0].x = 0;
            rects[0].y = 0;
            rects[0].data.1 = sheets.len();
            sheets.push((rects[0].w - pad, rects[0].h - pad));
        } else if rects.len() > 1 {
            let area = rects.iter().fold(0u64, |acc, ref rc| acc + (rc.w*rc.h).abs() as u64);
            let mut w = u32::next_power_of_two(f64::sqrt(area as f64).ceil().round() as u32) as i32;
            let mut h = w;

            while w <= max_size && h <= max_size && pack_rects(w + pad, h + pad, rects) < rects.len() {
                if w <= h {
                    w *= 2;
                } else {
                    h *= 2;
                }
            }

            if w <= max_size && h <= max_size {
                for rc in rects {
                    rc.data.1 = sheets.len();
                }

                sheets.push((w, h));
            } else {
                let area = area / 2;
                let mut i = 0;
                let mut a = 0;

                while a < area && i < rects.len() - 1 {
                    a = a + (rects[i].w*rects[i].h).abs() as u64;
                    i += 1;
                }

                let (rects1, rects2) = rects.split_at_mut(i);
                Self::pack_recursive(rects1, sheets, pad, max_size);
                Self::pack_recursive(rects2, sheets, pad, max_size);
            }
        }
    }
}
