#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io;

fn main() {
    let mut game = project::Game::default();

    let main = game.scripts.len() as i32;
    let main = format!("{}", main);
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
        sprite: -1,
        depth: 0,
        persistent: false,
        events: vec![
            project::Event {
                event_type: project::event_type::CREATE,
                event_kind: 0,
                actions: vec![
                    project::Action {
                        library: 1,
                        action: 601,
                        action_kind: project::action_kind::NORMAL,
                        has_target: true,
                        action_type: project::action_type::FUNCTION,
                        name: b"action_execute_script",
                        parameters_used: 6,
                        parameters: vec![
                            project::argument_type::SCRIPT,
                            project::argument_type::EXPR,
                            project::argument_type::EXPR,
                            project::argument_type::EXPR,
                            project::argument_type::EXPR,
                            project::argument_type::EXPR,
                        ],
                        target: gml::vm::SELF,
                        arguments: vec![
                            main.as_bytes(),
                            b"0", b"0", b"0", b"0", b"0",
                        ],
                        ..project::Action::default()
                    },
                ],
            },
        ],
    });

    let second_obj = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"second_obj",
        sprite: -1,
        depth: 0,
        persistent: false,
        events: vec![],
    });

    let _first_rm = game.rooms.len() as i32;

    game.last_instance += 1;
    let first_id = game.last_instance;

    game.last_instance += 1;
    let second_id = game.last_instance;

    game.rooms.push(project::Room {
        name: b"first_rm",
        code: b"",
        instances: vec![
            project::Instance { x: 0, y: 0, object_index: first_obj, id: first_id, code: b"" },
            project::Instance { x: 0, y: 0, object_index: second_obj, id: second_id, code: b"" },
        ],
    });

    let (assets, debug) = match runner::build(&game, io::stderr) {
        Ok(assets) => assets,
        Err(errors) => {
            if errors > 1 {
                eprintln!("aborting due to {} previous errors", errors);
            } else {
                eprintln!("aborting due to previous error");
            }
            return;
        }
    };
    let world = runner::World::from_assets(&assets, debug);
    runner::run(runner::Context { world, assets });
}
