import init, { run, stop } from "playground";
import wasm from "playground/playground_bg.wasm";
import { EditorState, basicSetup } from "@codemirror/basic-setup";
import { EditorView, keymap } from "@codemirror/view";
import { indentUnit } from "@codemirror/language";
import { defaultTabBinding } from "@codemirror/commands";
import { gml } from "codemirror-lang-gml";

(async () => {
  await init(wasm);
  run(view.state.sliceDoc());
})();

const doc = `// This script runs once per frame per object:

direction += 1

if x < 0 {
    hspeed = 3
} else if x > 256 {
    hspeed = -3
}

if y < 0 {
    vspeed = 3
} else if y > 384 {
    vspeed = -3
}`;

const state = EditorState.create({
  doc,
  extensions: [
    basicSetup,
    indentUnit.of("    "),
    keymap.of([
      defaultTabBinding,
      { key: "Mod-Enter", run(view) { run(view.state.sliceDoc()); return true; } },
    ]),
    gml(),
  ],
});
const view = new EditorView({ state, parent: document.getElementById("editor") });
document.getElementById("run").addEventListener("click", _ => run(view.state.sliceDoc()));
document.getElementById("stop").addEventListener("click", _ => stop());
