use std::collections::HashMap;
use std::io;

use gml::{Function, ErrorPrinter};
use gml::front::Span;
use engine::World;

fn main() {
    let mut game = project::Game::default();
    let mut items = HashMap::default();
    World::register(&mut items);

    let main = Function::Script { id: game.scripts.len() as i32 };
    game.scripts.push(project::Script { name: b"main", body: br#"{
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

        repeat (2) {
            show_debug_message()
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
            show_debug_message("instance_exists(id) =>", instance_exists(id))
            show_debug_message("instance_exists(100002) =>", instance_exists(100002))
            show_debug_message("instance_exists(2) =>", instance_exists(2))
            show_debug_message("instance_number(1) =>", instance_number(1))

            instance_destroy()
        }
    }"# });

    let first_obj = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"first_obj",
        persistent: false,
        events: vec![],
    });

    let second_obj = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"second_obj",
        persistent: false,
        events: vec![],
    });

    let (assets, debug) = engine::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let world = World::default();

    let mut thread = gml::vm::Thread::default();
    let mut cx = engine::Context { world, assets };

    let id = engine::instance::State::instance_create(&mut cx, &mut thread, 0.0, 0.0, first_obj)
        .unwrap_or_else(|_| panic!("object does not exist"));

    engine::instance::State::instance_create(&mut cx, &mut thread, 0.0, 0.0, second_obj)
        .unwrap_or_else(|_| panic!("object does not exist"));

    let engine::Context { world, .. } = &mut cx;
    let entity = world.world.instances[id];

    if let Err(error) = thread.with(entity).execute(&mut cx, main, vec![]) {
        let (mut errors, span, stack) = match error.frames[..] {
            [ref frame, ref stack @ ..] => {
                let errors = ErrorPrinter::from_debug(&debug, frame.function, io::stderr());
                let span = Span::from_debug(&debug, frame);
                (errors, span, stack)
            },
            _ => return,
        };

        ErrorPrinter::error(&mut errors, span, format_args!("{}", error.kind));
        ErrorPrinter::stack_from_debug(&mut errors, &debug, stack);
    }

    let engine::Context { world, .. } = &mut cx;
    world.instance.free_destroyed(&mut world.world, &mut world.motion);
}
