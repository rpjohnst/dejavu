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
    fn state(&self) -> (&vm::World, &instance::State) { (&self.world, &self.instance) }
    fn state_mut(&mut self) -> (&mut vm::World, &mut instance::State) {
        (&mut self.world, &mut self.instance)
    }
}

impl data::Api for Engine {
    fn state(&self) -> (&vm::World, &data::State) { (&self.world, &self.data) }
    fn state_mut(&mut self) -> (&mut vm::World, &mut data::State) {
        (&mut self.world, &mut self.data)
    }
}

impl Engine {
    fn show_debug_message(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        eprintln!("{:?}", arguments[0]);
        Ok(vm::Value::from(0))
    }
}

fn main() {
    let mut items = HashMap::new();

    <Engine as instance::Api>::register(&mut items);
    <Engine as data::Api>::register(&mut items);

    let show_debug_message = Symbol::intern("show_debug_message");
    items.insert(show_debug_message, gml::Item::Native(Engine::show_debug_message, 1, false));

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(r#"{
        var list;
        list = ds_list_create()
        ds_list_add(list, 3, "foo")
        ds_list_add(list, 5)
        show_debug_message("hello world")
        show_debug_message(ds_list_find_value(list, 0))
        show_debug_message(ds_list_find_value(list, 1))
        show_debug_message(ds_list_find_value(list, 2))
        show_debug_message(object_index)
        show_debug_message(id)
        show_debug_message(persistent)
        ds_list_destroy(list)
    }"#));

    let resources = gml::build(items);
    let mut engine = Engine::default();
    let mut thread = gml::vm::Thread::new();

    let object_index = 0;
    let id = 100001;
    let persistent = false;
    let entity = engine.world.create_instance(id);
    engine.instance.create_instance(entity, instance::Instance { object_index, id, persistent });
    thread.set_self(entity);

    let _ = thread.execute(&mut engine, &resources, main, &[]);
}
