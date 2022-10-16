use std::cmp;
use crate::atlas::Atlas;

mod atlas;

pub struct Texture {
    pub size: (u16, u16),
    pub data: Vec<u8>,
}

pub struct Image {
    pub texture: i32,
    pub pos: (u16, u16),
    pub size: (u16, u16),
}

#[derive(Default)]
pub struct Builder<'a> {
    images: Vec<Image>,
    data: Vec<&'a [u8]>,

    area: u32,
    max_width: u16,
    max_height: u16,
}

impl<'a> Builder<'a> {
    pub fn len(&self) -> usize { self.images.len() }

    pub fn insert(&mut self, size: (u32, u32), data: &'a [u8]) {
        let (width, height) = size;
        let size = (width as u16, height as u16);
        debug_assert!(width < u16::MAX as u32 && height < u16::MAX as u32);

        self.images.push(Image { texture: 0, pos: (0, 0), size });
        self.data.push(data);

        self.area += width * height;
        self.max_width = cmp::max(self.max_width, width as u16);
        self.max_height = cmp::max(self.max_height, height as u16);
    }

    pub fn build(mut self) -> (Vec<Texture>, Vec<Image>) {
        // As a heuristic, sort by height and assume the packer will achieve about 75% utilization.
        let mut image_index = Vec::from_iter(0..self.images.len());
        image_index.sort_by_key(|&image| {
            let Image { size: (_, height), .. } = self.images[image];
            cmp::Reverse(height)
        });
        let square = f32::sqrt(self.area as f32 / 0.75) as u16;
        let mut atlas_width = u16::next_power_of_two(cmp::max(self.max_width, square));
        let mut atlas_height = u16::next_power_of_two(cmp::max(self.max_height, square));
        let mut atlas = Atlas::new(atlas_width, atlas_height);
        'pack: loop {
            for &image in &image_index {
                let frame = &mut self.images[image];
                let (width, height) = frame.size;
                if let Some(pos) = atlas.pack(width, height) {
                    frame.texture = 0;
                    frame.pos = pos;
                } else {
                    // Something didn't fit. Move up to the next power of two and retry.
                    atlas_width *= 2;
                    atlas_height *= 2;
                    atlas.reset(atlas_width, atlas_height);
                    continue 'pack;
                }
            }
            break;
        }

        let len = atlas_width as usize * atlas_height as usize * 4;
        let mut texture = Vec::default();
        texture.resize_with(len, u8::default);
        for (image, &data) in self.data.iter().enumerate() {
            let Image { pos: (x, y), size: (width, _), .. } = self.images[image];
            for (i, row) in data.chunks_exact(width as usize * 4).enumerate() {
                let start = (y as usize + i) * (atlas_width as usize * 4) + (x as usize * 4);
                texture[start..start + row.len()].copy_from_slice(row);
            }
        }

        let texture = Texture { size: (atlas_width, atlas_height), data: texture };
        (vec![texture], self.images)
    }
}
