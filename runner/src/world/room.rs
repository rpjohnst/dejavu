use crate::{Context, instance, Instance};

use gml::vm;

#[derive(Default)]
pub struct State;

impl State {
    pub fn load_room(cx: &mut Context, thread: &mut vm::Thread, num: i32) ->
        vm::Result<()>
    {
        // Create instances:
        let Context { assets, .. } = cx;
        for i in 0..assets.rooms[num as usize].instances.len() {
            let Context { assets, .. } = cx;
            let Instance { x, y, object_index, id } = assets.rooms[0].instances[i];
            instance::State::instance_create_id(cx, x as f32, y as f32, object_index, id);
        }

        // Run each instance's creation code and create event:
        let Context { assets, .. } = cx;
        for i in 0..assets.rooms[num as usize].instances.len() {
            let Context { world, assets, .. } = cx;
            let Instance { object_index, id, .. } = assets.rooms[0].instances[i];
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

        crate::instance::State::free_destroyed(cx);
        Ok(())
    }
}
