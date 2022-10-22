use gml::{self, vm};
use atlas::Image;
use crate::{Context, Sprite, Background, Batch, batch};

#[derive(Default)]
pub struct State {
    pub platform: crate::platform::Draw,
    pub graphics: Option<crate::graphics::Draw>,
    pub instances: vm::EntityMap<Instance>,
    pub depth: Vec<vm::Entity>,
    pub batch: Batch,
}

pub struct Instance {
    pub sprite_index: i32,
    pub image_index: f32,
    pub image_speed: f32,
    pub depth: f32,
}

impl Default for Instance {
    fn default() -> Instance {
        Instance {
            sprite_index: 0,
            image_index: 0.0,
            image_speed: 1.0,
            depth: 0.0,
        }
    }
}

impl State {
    pub fn add_entity(&mut self, entity: vm::Entity, instance: Instance) {
        self.instances.insert(entity, instance);
        self.depth.push(entity);
    }

    pub fn free_destroyed(&mut self) {
        self.depth.retain(|&entity| self.instances.contains_key(entity));
    }

    pub fn draw(cx: &mut Context, thread: &mut vm::Thread) -> vm::Result<()> {
        State::screen_redraw(cx, thread)?;
        crate::instance::State::free_destroyed(cx);
        Ok(())
    }

    pub fn animate(cx: &mut Context) {
        let Context { world, assets, .. } = cx;
        let crate::World { world, room, draw, .. } = world;
        let entities = world.instances.values().clone();

        for &entity in entities.iter() {
            let draw = &mut draw.instances[entity];

            if draw.sprite_index != -1 {
                let image_number = assets.sprites[draw.sprite_index as usize].images.len();
                draw.image_index = (draw.image_index + draw.image_speed) % image_number as f32;
            }
        }

        for layer in &mut room.backgrounds[..] {
            layer.x += layer.hspeed;
            layer.y += layer.vspeed;
        }
    }

    fn draw_batch_image(
        cx: &mut Context, image: usize, position: batch::Rect, uv: batch::Rect
    ) {
        let Context { world, assets, .. } = cx;
        let crate::World { draw, .. } = world;
        let Image { texture, pos, size } = assets.images[image];
        if draw.batch.texture != texture || draw.batch.vertex.len() + 3 > u16::MAX as usize {
            crate::graphics::batch(cx);

            let Context { world, .. } = cx;
            let crate::World { draw, .. } = world;
            draw.batch.reset(texture);
        }

        // Subtract 0.5 from vertex positions to compensate for pixel sample positions.
        // GM does not account for viewport size here.
        let position = batch::Rect { x: position.x - 0.5, y: position.y - 0.5, ..position };
        let (x, y) = pos;
        let (w, h) = size;
        let image = batch::Rect { x: x as f32, y: y as f32, w: w as f32, h: h as f32 };

        let Context { world, .. } = cx;
        let crate::World { draw, .. } = world;
        draw.batch.quad(position, uv, image);
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

    #[gml::get(depth)]
    pub fn get_depth(&self, entity: vm::Entity) -> f32 { self.instances[entity].depth }
    #[gml::set(depth)]
    pub fn set_depth(&mut self, entity: vm::Entity, value: f32) {
        self.instances[entity].depth = value
    }

    #[gml::api]
    pub fn draw_sprite(
        cx: &mut Context, entity: vm::Entity,
        sprite: i32, mut subimg: i32, x: f32, y: f32
    ) {
        let Context { world, assets } = cx;
        let crate::World { draw, .. } = world;
        if sprite < 0 || assets.sprites.len() <= sprite as usize {
            return;
        }
        if subimg == -1 {
            subimg = vm::to_i32(draw.instances[entity].image_index as f64);
        }

        let Sprite { origin, ref images, .. } = assets.sprites[sprite as usize];
        let image = images.start + subimg as usize;
        let Image { size, .. } = assets.images[image];
        let (ox, oy) = origin;
        let (x, y) = (x - ox as f32, y - oy as f32);
        let (w, h) = size;
        let position = batch::Rect { x, y, w: w as f32, h: h as f32 };
        let uv = batch::Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
        Self::draw_batch_image(cx, image, position, uv);
    }

    #[gml::api]
    pub fn screen_redraw(cx: &mut Context, thread: &mut vm::Thread) -> vm::Result<()> {
        crate::graphics::frame(cx);

        let Context { world, .. } = cx;
        let crate::World { draw, .. } = world;
        draw.batch.reset(-1);

        let Context { world, .. } = cx;
        let crate::World { room, .. } = world;
        for i in 0..room.backgrounds.len() {
            let Context { world, assets } = cx;
            let crate::World { room, .. } = world;
            let (room_width, room_height) = assets.rooms[room.room as usize].size;
            let crate::room::Layer {
                visible, foreground, background, x, y, htiled, vtiled, xscale, yscale, ..
            } = room.backgrounds[i];

            if !visible || foreground { continue; }
            if background < 0 || assets.backgrounds.len() <= background as usize { continue; }

            let Background { image } = assets.backgrounds[background as usize];
            let Image { size: (w, h), .. } = assets.images[image];
            let (w, h) = (w as f32, h as f32);

            let (w, h) = (w * xscale, h * yscale);
            let (mut xstart, mut xend) = (x, x + w);
            if htiled {
                xstart = f32::rem_euclid(x, w);
                if xstart > 0.0 { xstart -= w; }
                xend = room_width as f32;
            }
            let (mut ystart, mut yend) = (y, y + h);
            if vtiled {
                ystart = f32::rem_euclid(y, h);
                if y > 0.0 { ystart -= h; }
                yend = room_height as f32;
            }

            let mut x = xstart;
            while x < xend {
                let mut y = ystart;
                while y < yend {
                    let position = batch::Rect { x, y, w, h };
                    let uv = batch::Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
                    Self::draw_batch_image(cx, image, position, uv);
                    y += h;
                }
                x += w;
            }
        }

        let Context { world, .. } = cx;
        let crate::World { draw, .. } = world;
        let instances = &draw.instances;
        draw.depth.sort_by(|&a, &b| {
            let a = instances[a].depth;
            let b = instances[b].depth;
            f32::partial_cmp(&b, &a)
                .unwrap_or_else(|| bool::cmp(&!a.is_nan(), &!b.is_nan()))
        });

        for i in 0..draw.depth.len() {
            let Context { world, assets } = cx;
            let crate::World { motion, instance, draw, .. } = world;
            let entity = draw.depth[i];
            let &crate::motion::Instance { x, y, .. } = &motion.instances[entity];
            let &crate::instance::Instance { object_index, .. } = &instance.instances[entity];
            let &Instance { sprite_index, image_index, .. } = &draw.instances[entity];

            let event_type = project::event_type::DRAW;
            let event_kind = project::event_kind::DRAW;
            let draw = gml::Function::Event { event_type, event_kind, object_index };
            if assets.code.code.contains_key(&draw) {
                thread.with(entity).execute(cx, draw, vec![])?;
            } else {
                Self::draw_sprite(cx, entity, sprite_index, vm::to_i32(image_index as f64), x, y);
            }
        }
        crate::graphics::batch(cx);

        crate::graphics::present(cx);
        Ok(())
    }

    #[gml::api]
    pub fn action_draw_sprite(
        cx: &mut Context, entity: vm::Entity, relative: bool,
        sprite: i32, mut x: f32, mut y: f32, subimg: i32
    ) {
        let Context { world, .. } = cx;
        if relative {
            x += world.motion.instances[entity].x;
            y += world.motion.instances[entity].y;
        }

        State::draw_sprite(cx, entity, sprite, subimg, x, y);
    }
}
