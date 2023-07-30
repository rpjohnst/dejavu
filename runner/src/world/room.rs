use crate::{Context, World, Instance, instance};

use gml::vm;

#[derive(Default)]
pub struct State {
    pub room: i32,
    pub backgrounds: Vec<Layer>,
}

pub struct Layer {
    pub visible: bool,
    pub foreground: bool,
    pub background: i32,
    pub x: f32,
    pub y: f32,
    pub htiled: bool,
    pub vtiled: bool,
    pub xscale: f32,
    pub yscale: f32,
    pub hspeed: f32,
    pub vspeed: f32,
}

impl State {
    pub fn load_room(cx: &mut Context, thread: &mut vm::Thread, num: i32) ->
        vm::Result<()>
    {
        let Context { world, assets, .. } = cx;
        let World { room, .. } = world;
        room.room = num;

        room.backgrounds.clear();
        room.backgrounds.extend(assets.rooms[num as usize].backgrounds.iter().map(|&crate::Layer {
            visible, foreground, background, x, y, htiled, vtiled, xscale, yscale, hspeed, vspeed
        }| Layer {
            visible, foreground, background,
            x: x as f32, y: y as f32, htiled, vtiled, xscale, yscale,
            hspeed: hspeed as f32, vspeed: vspeed as f32
        }));

        // Create instances:
        for i in 0..assets.rooms[num as usize].instances.len() {
            let Context { assets, .. } = cx;
            let Instance { x, y, object_index, id } = assets.rooms[num as usize].instances[i];
            instance::State::instance_create_id(cx, x as f32, y as f32, object_index, id);
        }

        // Run each instance's creation code and create event:
        let Context { assets, .. } = cx;
        for i in 0..assets.rooms[num as usize].instances.len() {
            let Context { world, assets, .. } = cx;
            let Instance { object_index, id, .. } = assets.rooms[num as usize].instances[i];
            let crate::World { world, .. } = world;
            let entity = world.instances[id];

            let Context { assets, .. } = cx;
            let create = gml::Function::Instance { id };
            if assets.code.code.contains_key(&create) {
                thread.with(entity).execute(cx, create, vec![])?;
            }

            let Context { assets, .. } = cx;
            let event_type = project::event_type::CREATE;
            let create = gml::Function::Event { event_type, event_kind: 0, object_index };
            if assets.code.code.contains_key(&create) {
                thread.with(entity).execute(cx, create, vec![])?;
            }
        }

        // Run the room's creation code:
        let Context { assets, .. } = cx;
        let create = gml::Function::Room { id: num };
        if assets.code.code.contains_key(&create) {
            thread.execute(cx, create, vec![])?;
        }

        // Run room start events:
        let Context { world, .. } = cx;
        let crate::World { world, .. } = world;
        let entities = world.instances.values().clone();
        for &entity in entities.iter() {
            let Context { world, assets, .. } = cx;
            let crate::World { instance, .. } = world;
            let &instance::Instance { object_index, .. } = &instance.instances[entity];

            let event_type = project::event_type::OTHER;
            let event_kind = project::event_kind::ROOM_START;
            let step = gml::Function::Event { event_type, event_kind, object_index };
            if assets.code.code.contains_key(&step) {
                thread.with(entity).execute(cx, step, vec![])?;
            }
        }

        instance::State::free_destroyed(cx);
        Ok(())
    }
}

#[gml::bind]
impl State {
    #[gml::get(room)]
    pub fn get_room(&self) -> i32 { self.room }

    #[gml::get(room_width)]
    pub fn get_room_width(cx: &Context) -> u32 {
        let Context { world, assets } = cx;
        let World { room, .. } = world;
        let (width, _) = assets.rooms[room.room as usize].size;
        width
    }

    #[gml::get(room_height)]
    pub fn get_room_height(cx: &Context) -> u32 {
        let Context { world, assets } = cx;
        let World { room, .. } = world;
        let (_, height) = assets.rooms[room.room as usize].size;
        height
    }
}
