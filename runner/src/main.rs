use std::collections::HashMap;

use gml::front::{Span, ErrorHandler, Lines, ErrorPrinter};
use gml::symbol::Symbol;
use engine::{Engine, instance::Instance};

fn main() {
    let mut items = HashMap::new();
    Engine::register(&mut items);

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(br#"{
        show_debug_message("hello world")

        var list;
        list = ds_list_create()
        ds_list_add(list, 3, "foo")
        ds_list_add(list, 5)
        show_debug_message("list[| 0] =", ds_list_find_value(list, 0))
        show_debug_message("list[| 1] =", ds_list_find_value(list, 1))
        show_debug_message("list[| 2] =", ds_list_find_value(list, 2))
        ds_list_destroy(list)

        var map, key1, key2;
        map = ds_map_create()
        ds_map_add(map, 3, "foo")
        ds_map_add(map, "abc", "bar")
        key1 = ds_map_find_first(map)
        key2 = ds_map_find_next(map, key1)
        show_debug_message("map[?", key1, "] =", ds_map_find_value(map, key1))
        show_debug_message("map[?", key2, "] =", ds_map_find_value(map, key2))
        ds_map_destroy(map)

        var grid;
        grid = ds_grid_create(2, 2)
        ds_grid_set(grid, 0, 0, 1)
        ds_grid_set(grid, 1, 0, 2)
        ds_grid_set(grid, 0, 1, 3)
        ds_grid_set(grid, 1, 1, 4)
        show_debug_message("grid[# 0, 0] =", ds_grid_get(grid, 0, 0))
        show_debug_message("grid[# 1, 0] =", ds_grid_get(grid, 1, 0))
        show_debug_message("grid[# 0, 1] =", ds_grid_get(grid, 0, 1))
        show_debug_message("grid[# 1, 1] =", ds_grid_get(grid, 1, 1))
        ds_grid_destroy(grid)

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
        show_debug_message("instance_number(1) =>", instance_number(1))
    }"#));

    let resources = gml::build(&items).unwrap_or_else(|_| panic!());
    let mut engine = Engine::default();
    let mut thread = gml::vm::Thread::new();

    let object_index = 0;
    let id = 100001;
    let persistent = false;
    let entity = engine.world.create_instance(object_index, id);
    engine.instance.create_instance(entity, Instance { object_index, id, persistent });
    thread.set_self(entity);

    let object_index = 1;
    let id = 100002;
    let persistent = false;
    let entity = engine.world.create_instance(object_index, id);
    engine.instance.create_instance(entity, Instance { object_index, id, persistent });

    if let Err(error) = thread.execute(&mut engine, &resources, main, &[]) {
        let location = resources.debug[&error.symbol].get_location(error.instruction as u32);
        let lines = match items[&error.symbol] {
            gml::Item::Event(source) => Lines::from_event(source),
            gml::Item::Script(source) => Lines::from_script(source),
            _ => Lines::from_script(b""),
        };
        let mut errors = ErrorPrinter::new(error.symbol, lines);
        let span = Span { low: location as usize, high: location as usize };
        errors.error(span, &format!("{}", error.kind));
    }
}
