extern crate gml;
extern crate lib;

use std::collections::HashMap;

use gml::{symbol::Symbol, vm};
use lib::{data, instance};

#[derive(Default)]
struct Engine {
    world: vm::World,
    instance: instance::State,
    data: data::State,
}

impl vm::world::Api for Engine {
    fn state(&self) -> &vm::World { &self.world }
    fn state_mut(&mut self) -> &mut vm::World { &mut self.world }
}

impl instance::Api for Engine {
    fn state(&self) -> (&instance::State, &vm::World) { (&self.instance, &self.world) }
    fn state_mut(&mut self) -> (&mut instance::State, &mut vm::World) {
        (&mut self.instance, &mut self.world)
    }
}

impl data::Api for Engine {
    fn state(&self) -> (&data::State, &vm::World) { (&self.data, &self.world) }
    fn state_mut(&mut self) -> (&mut data::State, &mut vm::World) {
        (&mut self.data, &mut self.world)
    }
}

impl Engine {
    fn show_debug_message(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        for argument in arguments {
            eprint!("{:?} ", argument);
        }
        eprintln!();
        Ok(vm::Value::from(0))
    }
}

fn main() {
    let mut items = HashMap::new();

    <Engine as instance::Api>::register(&mut items);
    <Engine as data::Api>::register(&mut items);

    let show_debug_message = Symbol::intern("show_debug_message");
    items.insert(show_debug_message, gml::Item::Native(Engine::show_debug_message, 0, true));

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(r#"{
        show_debug_message("hello world")

        var list;
        list = ds_list_create()
        ds_list_add(list, 3, "foo")
        ds_list_add(list, 5)
        show_debug_message("list[| 0] =", ds_list_find_value(list, 0))
        show_debug_message("list[| 1] =", ds_list_find_value(list, 1))
        show_debug_message("list[| 2] =", ds_list_find_value(list, 2))
        ds_list_destroy(list)

        show_debug_message("object_index =", object_index)
        show_debug_message("id =", id)
        persistent = true
        show_debug_message("persistent =", persistent)

        for (i = 0; i < instance_count; i += 1) {
            if instance_exists(instance_id[i]) {
                show_debug_message(instance_id[i], "=>", instance_id[i].object_index)
            }
        }

        show_debug_message("instance_find(1, 0) =>", instance_find(1, 0))
        show_debug_message("instance_exists(0) =>", instance_exists(0))
        show_debug_message("instance_exists(100002) =>", instance_exists(100002))
        show_debug_message("instance_exists(2) =>", instance_exists(2))
    }"#));

    let resources = gml::build(items);
    let mut engine = Engine::default();
    let mut thread = gml::vm::Thread::new();

    let object_index = 0;
    let id = 100001;
    let persistent = false;
    let entity = engine.world.create_instance(object_index, id);
    engine.instance.create_instance(entity, instance::Instance { object_index, id, persistent });
    thread.set_self(entity);

    let object_index = 1;
    let id = 100002;
    let persistent = false;
    let entity = engine.world.create_instance(object_index, id);
    engine.instance.create_instance(entity, instance::Instance { object_index, id, persistent });

    let _ = thread.execute(&mut engine, &resources, main, &[]);
}
