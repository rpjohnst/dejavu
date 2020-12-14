use gml::{self, vm};
use crate::{Context, Texture, Frame, Batch, batch};

#[derive(Default)]
pub struct State {
    pub platform: crate::platform::Draw,
    pub graphics: Option<crate::graphics::Draw>,
    pub instances: vm::EntityMap<Instance>,
    pub batch: Batch,
}

#[derive(Default)]
pub struct Instance {
    pub sprite_index: i32,
    pub image_index: f32,
}

impl State {
    pub fn draw_world(cx: &mut Context) {
        let Context { world, .. } = cx;
        let crate::World { world, draw, .. } = world;
        let entities = world.instances.values().clone();

        let mut active_texture = draw.batch.texture;
        for &entity in entities.iter() {
            let Context { world, assets } = cx;
            let crate::World { motion, draw, .. } = world;
            let &crate::motion::Instance { x, y, .. } = &motion.instances[entity];
            let &Instance { sprite_index, image_index, .. } = &draw.instances[entity];
            if sprite_index < 0 {
                continue;
            }

            let sprite = &assets.sprites[sprite_index as usize];
            let &Frame { texture, pos, size } = &sprite.frames[image_index as usize];
            let &Texture { size: texture_size, .. } = &assets.textures[texture as usize];
            if active_texture != texture {
                crate::graphics::batch(cx);

                let Context { world, .. } = cx;
                let crate::World { draw, .. } = world;
                draw.batch.reset(texture, texture_size);
                active_texture = texture;
            }

            let Context { world, .. } = cx;
            let crate::World { draw, .. } = world;
            let (w, h) = size;
            let world = batch::Rect { x, y, w: w as f32, h: h as f32 };
            let (x, y) = pos;
            let texture = batch::Rect { x: x as f32, y: y as f32, w: w as f32, h: h as f32 };
            draw.batch.quad(world, texture);
        }

        crate::graphics::batch(cx);

        let Context { world, .. } = cx;
        let crate::World { draw, .. } = world;
        draw.batch.reset(-1, (0, 0));
    }
}
