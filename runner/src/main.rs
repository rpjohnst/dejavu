extern crate gml;
extern crate lib;

use std::collections::HashMap;

use gml::{symbol::Symbol, vm};
use lib::{data};

#[derive(Default)]
struct Engine {
    world: vm::World,
    data: data::State,
}

impl vm::world::Api for Engine {
    fn state(&mut self) -> &mut vm::World { &mut self.world }
}

impl data::Api for Engine {
    fn state(&mut self) -> &mut data::State { &mut self.data }
}

impl Engine {
    fn show_debug_message(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        eprintln!("{:?}", arguments[0]);
        Ok(vm::Value::from(0))
    }
}

fn main() {
    let mut items = HashMap::new();

    <Engine as data::Api>::register(&mut items);

    let show_debug_message = Symbol::intern("show_debug_message");
    items.insert(show_debug_message, gml::Item::Native(Engine::show_debug_message, 1, false));

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(r#"{
        var list;
        list = ds_list_create()
        ds_list_add(list, 3)
        ds_list_add(list, "foo")
        show_debug_message("hello world")
        show_debug_message(ds_list_find_value(list, 0))
        show_debug_message(ds_list_find_value(list, 1))
        ds_list_destroy(list)
    }"#));

    let resources = gml::build(items);
    let mut engine = Engine::default();
    let mut thread = gml::vm::Thread::new();

    let _ = thread.execute(&mut engine, &resources, main, &[]);
}
