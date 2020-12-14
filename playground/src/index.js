import init, { run, stop } from "playground";
import wasm from "playground/playground_bg.wasm";
import ace from "ace-builds";

(async () => {
  await init(wasm);
  run(editor.getValue());
})();

ace.config.set("basePath", "module");
const editor = ace.edit("editor", {
  useSoftTabs: true,
  navigateWithinSoftTabs: true,

  scrollPastEnd: 1
});
editor.setShowPrintMargin(false);

document.getElementById("run").addEventListener("click", event => run(editor.getValue()));
document.getElementById("stop").addEventListener("click", event => stop());
editor.commands.addCommand({
  name: "run",
  bindKey: { win: "Ctrl-Enter", mac: "Command-Enter" },
  exec: editor => run(editor.getValue()),
});
