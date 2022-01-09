import init, { run, stop } from "./playground.js";
import { EditorState, basicSetup } from "@codemirror/basic-setup";
import { EditorView, keymap } from "@codemirror/view";
import { indentUnit } from "@codemirror/language";
import { indentWithTab } from "@codemirror/commands";
import { gml } from "codemirror-lang-gml";

(async () => {
  const canvas = document.getElementById("canvas");
  const output = document.getElementById("output");
  await init(canvas, output);
})();

// Model

const canvas = document.createElement("canvas");
canvas.width = 32;
canvas.height = 32;
const cx = canvas.getContext("2d", { desynchronized: true });
cx.translate(0.5, 0.5);
cx.fillStyle = "#EDEDED";
cx.beginPath();
cx.arc(15, 15, 15, 0, 2 * Math.PI);
cx.fill();
cx.fillStyle = "#CE0000";
cx.beginPath();
cx.arc(15, 15, 13, 0, 2 * Math.PI);
cx.fill();

let project = {
  sprites: [
    {
      name: "circle_spr",
      origin: [0, 0],
      frames: [
        { size: [32, 32], canvas },
      ],
      masks: [],
    },
  ],
  objects: [
    {
      name: "playground_obj",
      sprite: 0,
      events: [
        {
          event_type: 0,
          event_kind: 0,
          actions: [
            {
              library: 1,
              action: 603,
              action_kind: 7,
              has_target: true,
              parameters_used: 1,
              parameters: [1],
              target: -1,
              arguments: [""],
            },
          ],
        },
        {
          event_type: 3,
          event_kind: 0,
          actions: [
            {
              library: 1,
              action: 603,
              action_kind: 7,
              parameters_used: 1,
              parameters: [1],
              target: -1,
              arguments: [`direction += 1

if x < 0 {
    hspeed = 3
} else if x > 256 {
    hspeed = -3
}

if y < 0 {
    vspeed = 3
} else if y > 384 {
    vspeed = -3
}`],
            },
          ],
        },
      ],
    },
  ],
  rooms: [
    {
      name: "playground_rm",
      code: "",
      instances: [
        { x: 85, y: 128, object_index: 0, id: 100001, code: "hspeed = 3; vspeed = -3;" },
        { x: 171, y: 128, object_index: 0, id: 100002, code: "hspeed = -3; vspeed = 3;" },
      ]
    },
  ],
  last_instance: 100002,
};

// View

let editor = {
  resource: undefined,
  view: undefined,
  apply() {},
  destroy() {},
};

document.getElementById("run").addEventListener("click", event => {
  event.preventDefault();
  start();
});
document.getElementById("stop").addEventListener("click", event => {
  event.preventDefault();
  stop();
});
const resourcesView = document.getElementById("resources");
const editorView = document.getElementById("editor");

const ul = resourcesView.appendChild(document.createElement("ul"));
ul.appendChild(resourceTree("Sprites", project.sprites, spriteOnEdit));
ul.appendChild(resourceTree("Objects", project.objects, objectOnEdit));
ul.appendChild(resourceTree("Rooms", project.rooms, roomOnEdit));

function resourceTree(name, data, resourceOnEdit) {
  const li = document.createElement("li");
  li.classList.add("open");

  const a = li.appendChild(document.createElement("a"));
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

  const resources = li.appendChild(document.createElement("ul"));
  for (const resource of data) {
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

  return li;
}

function spriteOnEdit(parent, sprite) {
  const view = parent.appendChild(document.createElement("div"));
  const name = nameOnEdit(view, sprite);

  const sheet = view.appendChild(document.createElement("div"));
  sheet.classList.add("sheet");
  sheet.appendChild(sprite.frames[0].canvas);

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
  sprite.appendChild(project.sprites[object.sprite].frames[0].canvas);

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
  const kindNames = eventNames[event.event_type];
  kind.textContent = kindNames instanceof Array ?
    kindNames[event.event_kind] :
    kindNames(event.event_kind);

  const doc = event.actions[0].arguments[0];
  const state = EditorState.create({ doc, extensions });
  const editor = new EditorView({ state, parent: view });

  return {
    resource: event,
    editor,
    apply() { this.resource.actions[0].arguments[0] = this.editor.state.sliceDoc(); },
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

function start() {
  editor.apply();

  run((builder) => {
    for (const sprite of project.sprites) {
      const [width, height] = sprite.frames[0].size;
      const cx = sprite.frames[0].canvas.getContext("2d");
      const data = cx.getImageData(0, 0, width, height).data;
      builder.sprite(sprite.name, sprite.origin, [width, height], data);
    }

    for (const object of project.objects) {
      builder.object(object.name, object.sprite, object.events.length);
      for (const event of object.events) {
        builder.event(event.event_type, event.event_kind, event.actions[0].arguments[0]);
      }
    }

    for (const room of project.rooms) {
      builder.room(room.name, room.instances[0].object_index);
    }
  });
}
