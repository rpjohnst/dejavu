import init, { run } from "playground";
import wasm from "playground/playground_bg.wasm";
import { clear } from "./print.js";
import ace from "ace-builds";

(async () => {
  try {
    await init(wasm);
  } catch (error) {
    console.error(error);
  }
})();

ace.config.set("basePath", "module");
const editor = ace.edit("editor", {
  useSoftTabs: true,
  navigateWithinSoftTabs: true,

  scrollPastEnd: 1
});
editor.setShowPrintMargin(false);

document.getElementById("run").addEventListener("click", event => exec(editor.getValue()));
editor.commands.addCommand({
  name: "run",
  bindKey: { win: "Ctrl-Enter", mac: "Command-Enter" },
  exec: editor => exec(editor.getValue()),
});

function exec(string) {
  clear();
  run(string);
}

/*
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
*/
