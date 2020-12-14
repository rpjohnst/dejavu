use gml::{self, vm};
use crate::{Context, Texture, Sprite, Frame, Batch, batch};

#[derive(Default)]
pub struct State {
    pub platform: crate::platform::Draw,
    pub graphics: Option<crate::graphics::Draw>,
    pub instances: vm::EntityMap<Instance>,
    pub batch: Batch,
}

pub struct Instance {
    pub sprite_index: i32,
    pub image_index: f32,
    pub image_speed: f32,
}

impl Default for Instance {
    fn default() -> Instance {
        Instance {
            sprite_index: 0,
            image_index: 0.0,
            image_speed: 1.0,
        }
    }
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

            let &Sprite { origin, ref frames, .. } = &assets.sprites[sprite_index as usize];
            let &Frame { texture, pos, size } = &frames[image_index as usize];
            let &Texture { size: texture_size, .. } = &assets.textures[texture as usize];
            if active_texture != texture {
                crate::graphics::batch(cx);

                let Context { world, .. } = cx;
                let crate::World { draw, .. } = world;
                draw.batch.reset(texture, texture_size);
                active_texture = texture;
            }

            let (ox, oy) = origin;
            let (x, y) = (x - ox as f32, y - oy as f32);
            let (w, h) = size;
            let position = batch::Rect { x, y, w: w as f32, h: h as f32 };
            let (x, y) = pos;
            let texture = batch::Rect { x: x as f32, y: y as f32, w: w as f32, h: h as f32 };

            let Context { world, .. } = cx;
            let crate::World { draw, .. } = world;
            draw.batch.quad(position, texture);
        }

        crate::graphics::batch(cx);

        let Context { world, .. } = cx;
        let crate::World { draw, .. } = world;
        draw.batch.reset(-1, (0, 0));
    }
}

#[gml::bind]
impl State {
    #[gml::get(sprite_index)]
    pub fn get_sprite_index(&self, entity: vm::Entity) -> i32 {
        self.instances[entity].sprite_index
    }
    #[gml::set(sprite_index)]
    pub fn set_sprite_index(&mut self, entity: vm::Entity, value: i32) {
        self.instances[entity].sprite_index = value
    }

    #[gml::get(image_index)]
    pub fn get_image_index(&self, entity: vm::Entity) -> f32 { self.instances[entity].image_index }
    #[gml::set(image_index)]
    pub fn set_image_index(&mut self, entity: vm::Entity, value: f32) {
        self.instances[entity].image_index = value
    }

    #[gml::get(image_speed)]
    pub fn get_image_speed(&self, entity: vm::Entity) -> f32 { self.instances[entity].image_speed }
    #[gml::set(image_speed)]
    pub fn set_image_speed(&mut self, entity: vm::Entity, value: f32) {
        self.instances[entity].image_speed = value
    }
}
