import init, { with_game, end } from "./playground.js";
import { basicSetup } from "codemirror";
import { EditorView, keymap } from "@codemirror/view";
import { indentUnit } from "@codemirror/language";
import { indentWithTab } from "@codemirror/commands";
import { gml } from "codemirror-lang-gml";

(async () => {
  const canvas = document.getElementById("canvas");
  const output = document.getElementById("output");
  await init(canvas, output);
  open(new Blob());
})();

// Model

let buffer = new ArrayBuffer();
let project = { sprites: [], objects: [], rooms: [] };
let state = 0;

// View

let resources = {
  view: undefined,
  destroy() {},
};
let editor = {
  resource: undefined,
  view: undefined,
  apply() {},
  destroy() {},
};

document.getElementById("open").addEventListener("click", event => {
  event.preventDefault();
  document.getElementById("open-input").click();
});
document.getElementById("open-input").addEventListener("change", event => {
  open(event.target.files[0]);
});
document.getElementById("start").addEventListener("click", event => {
  event.preventDefault();
  start();
});
document.getElementById("stop").addEventListener("click", event => {
  event.preventDefault();
  stop();
});
const resourcesView = document.getElementById("resources");
const editorView = document.getElementById("editor");
const gameView = document.getElementById("game");

gameView.style.display = "none";

function projectOnOpen() {
  const view = resourcesView.appendChild(document.createElement("ul"));

  resourceTree(spriteOnEdit, view, "Sprites", project.sprites);
  resourceTree(objectOnEdit, view, "Objects", project.objects);
  resourceTree(roomOnEdit, view, "Rooms", project.rooms);

  return {
    view,
    destroy() { this.view.remove(); },
  };
}

function resourceTree(resourceOnEdit, parent, name, data) {
  const view = parent.appendChild(document.createElement("li"));
  view.classList.add("open");

  const a = view.appendChild(document.createElement("a"));
  a.href = "#";
  a.textContent = name;
  a.addEventListener("click", event => {
    event.preventDefault();
    const li = event.target.parentElement;
    if (li.classList.contains("open")) {
      li.classList.remove("open");
    } else {
      li.classList.add("open");
    }
  });

  const resources = view.appendChild(document.createElement("ul"));
  for (const index in data) {
    const resource = data[index];
    const li = resources.appendChild(document.createElement("li"));
    const a = li.appendChild(document.createElement("a"));
    a.href = "#";
    a.textContent = resource.name;
    a.addEventListener("click", event => {
      event.preventDefault();

      if (editor.resource == resource) { return; }
      editor.apply();
      editor.destroy();
      editor = undefined;

      editor = resourceOnEdit(editorView, resource);
      editor.view.addEventListener("rename", _ => {
        editor.name.apply();
        a.textContent = resource.name;
      });
    });
  }
}

function spriteOnEdit(parent, sprite) {
  const view = parent.appendChild(document.createElement("div"));
  const name = nameOnEdit(view, sprite);

  const sheet = view.appendChild(document.createElement("div"));
  sheet.classList.add("sheet");
  sheet.appendChild(sprite.canvas);

  return {
    resource: sprite,
    view,
    name,
    apply() { this.name.apply(); },
    destroy() { this.view.remove(); },
  };
}

function objectOnEdit(parent, object) {
  const view = parent.appendChild(document.createElement("div"));
  const name = nameOnEdit(view, object);

  const sprite = view.appendChild(document.createElement("div"));
  sprite.classList.add("sprite");
  sprite.textContent = "Sprite: ";
  if (project.sprites[object.sprite]) {
    sprite.appendChild(project.sprites[object.sprite].canvas);
  }

  const events = [];
  for (const event of object.events) { events.push(eventOnEdit(view, event)); }

  return {
    resource: object,
    view,
    name,
    events,
    apply() {
      this.name.apply();
      for (const event of this.events) { event.apply(); }
    },
    destroy() {
      for (const event of this.events) { event.destroy(); }
      this.view.remove();
    },
  };
}

const eventNames = [
  ["Create"],
  ["Destroy"],
  i => `Alarm ${i}`,
  ["Step", "Begin Step", "End Step"],
  i => `Collision with ${project.objects[i].name}`,
  i => `Keyboard ${i}`,
  [
    "Left Mouse Button", "Right Mouse Button", "Middle Mouse Button", "No Mouse Button",
    "Left Mouse Button Pressed", "Right Mouse Button Pressed", "Middle Mouse Button Pressed",
    "Left Mouse Button Released", "Right Mouse Button Released", "Middle Mouse Button Released",
    "Mouse Enter", "Mouse Leave",
    "Global Left Mouse Button", "Global Right Mouse Button", "Global Middle Mouse Button",
    "Global Left Mouse Button Pressed", "Global Right Mouse Button Pressed", "Global Middle Mouse Button Pressed",
    "Global Left Mouse Button Released", "Global Right Mouse Button Released", "Global Middle Mouse Button Released",
    "Mouse Wheel Up",
    "Mouse Wheel Down",
  ],
  [
    "Outside Room", "Intersect Boundary",
    "Game Start", "Game End",
    "Room Start", "Room End",
    "No More Lives",
    "Animation End", "Path End",
    "No More Health",
    "User 0", "User 1", "User 2", "User 3", "User 4", "User 5", "User 6", "User 7",
    "User 8", "User 9", "User 10", "User 11", "User 12", "User 13", "User 14", "User 15",
    "Close Button",
    "Outside View",
    "Intersect View Boundary",
  ],
  ["Draw", "Draw GUI", "Draw Resize"],
  i => `Keyboard Press ${i}`,
  i => `Keyboard Release ${i}`,
  i => `Trigger ${i}`,
];
const extensions = [
  basicSetup,
  indentUnit.of("    "),
  keymap.of([
    indentWithTab,
    { key: "Mod-Enter", run(view) { start(); return true; } },
  ]),
  gml(),
];
function eventOnEdit(parent, event) {
  const view = parent.appendChild(document.createElement("div"));

  const label = view.appendChild(document.createElement("label"));
  label.textContent = "Event: ";
  const kind = label.appendChild(document.createElement("output"));
  const kindNames = eventNames[event.type];
  kind.textContent = kindNames instanceof Array ?
    kindNames[event.kind] :
    kindNames(event.kind);

  const doc = event.code;
  const editor = new EditorView({ doc, extensions, parent: view });

  return {
    resource: event,
    editor,
    apply() { this.resource.code = this.editor.state.sliceDoc(); },
    destroy() { this.editor.destroy(); },
  }
}

function roomOnEdit(parent, room) {
  const view = parent.appendChild(document.createElement("div"));
  const name = nameOnEdit(view, room);

  return {
    resource: room,
    view,
    name,
    apply() { this.name.apply(); },
    destroy() { this.view.remove(); },
  }
}

function nameOnEdit(parent, resource) {
  const label = parent.appendChild(document.createElement("label"));
  label.textContent = "Name: ";

  const name = label.appendChild(document.createElement("input"));
  name.value = resource.name;
  name.addEventListener("change", _ => parent.dispatchEvent(new Event("rename")));

  return {
    resource,
    name,
    apply() { this.resource.name = this.name.value; }
  };
}

// Controller

async function open(blob) {
  stop();

  editor.destroy();
  editor = { resource: undefined, view: undefined, apply() {}, destroy() {} };

  resources.destroy();
  resources = undefined;

  buffer = undefined;
  project = { sprites: [], objects: [], rooms: [] };

  buffer = await blob.arrayBuffer();
  with_game((game) => {
    game.read_project(new Uint8Array(buffer));

    game.visit({
      sprite(sprite, name, [width, height], data) {
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        const cx = canvas.getContext("2d", { desynchronized: true });
        const pixels = new Uint8ClampedArray(data);
        const target = new Uint32Array(pixels.buffer, pixels.byteOffset, width * height);
        for (let i = 0; i < width * height; i++) {
          target[i] =
            (data[4 * i + 2] << 0) |
            (data[4 * i + 1] << 8) |
            (data[4 * i + 0] << 16) |
            (data[4 * i + 3] << 24);
        }
        cx.putImageData(new ImageData(pixels, width, height), 0, 0);
        project.sprites[sprite] = { name, canvas };
      },

      object(object, name, sprite) {
        project.objects[object] = { name, sprite, events: [] };
      },
      event(object, type, kind, code) {
        project.objects[object].events.push({ type, kind, code });
      },

      room(room_index, name) {
        project.rooms[room_index] = { name };
      },
    });
  });
  resources = projectOnOpen();
}

function start() {
  stop();

  editor.apply();

  gameView.style.display = "";
  with_game((game) => {
    game.read_project(new Uint8Array(buffer));

    for (const [sprite, { name }] of project.sprites.entries()) {
      game.sprite(sprite, name);
    }

    for (const [object, { name, events }] of project.objects.entries()) {
      game.object(object, name);
      for (const { type, kind, code } of events) {
        game.event(object, type, kind, code);
      }
    }

    for (const [room, { name }] of project.rooms.entries()) {
      game.room(room, name);
    }

    state = game.run();
  });
}

function stop() {
  end(state);
  state = 0;
  gameView.style.display = "none";
}
