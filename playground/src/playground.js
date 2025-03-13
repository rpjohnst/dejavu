import { clear, outPrint, errPrint } from "./page.js";
import { schedule, cancel } from "../../runner/src/platform/web.js";
import { rendererNew, rendererFrame, rendererBatch } from "../../runner/src/graphics/webgl2.js";
import playground_wasm from "./playground.wasm";
let playground;

let canvasRef;

export default async function init(canvas, output) {
  const imports = {};
  const env = imports.env = {};

  env.with_game_call = (fn, arena, game) => {
    fn = deref(fn);
    game = new Game(arena, game);
    try {
      fn(game);
    } finally {
      game.drop();
    }
  };
  env.visit_sprite = (visitor, sprite, namePtr, nameLen, sizeX, sizeY, dataPtr, dataLen) => {
    visitor = deref(visitor);
    const name = stringFromWasm(namePtr, nameLen);
    const data = sliceU8FromWasm(dataPtr, dataLen);
    visitor.sprite(sprite, name, [sizeX, sizeY], data);
  };
  env.visit_object = (visitor, object, namePtr, nameLen, sprite) => {
    visitor = deref(visitor);
    const name = stringFromWasm(namePtr, nameLen);
    visitor.object(object, name, sprite);
  };
  env.visit_object_event = (visitor, object, type, kind, codePtr, codeLen) => {
    visitor = deref(visitor);
    const code = stringFromWasm(codePtr, codeLen);
    visitor.event(object, type, kind, code);
  };
  env.visit_room = (visitor, room, namePtr, nameLen) => {
    visitor = deref(visitor);
    const name = stringFromWasm(namePtr, nameLen);
    visitor.room(room, name);
  };

  canvasRef = alloc(canvas);

  env.clear = () => clear(output);
  env.out_print = (ptr, len) => outPrint(output, stringFromWasm(ptr, len));
  env.err_print = (ptr, len) => errPrint(output, stringFromWasm(ptr, len));

  env.schedule = (fn, cx) => schedule(() => playground.__indirect_function_table.get(fn)(cx));
  env.cancel = cancel;

  env.renderer_new = (canvas, atlasPtr, atlasLen, width, height) => {
    canvas = deref(canvas);
    const atlas = sliceU8FromWasm(atlasPtr, atlasLen);
    const renderer = rendererNew(canvas, atlas, width, height);
    return alloc(renderer);
  };
  env.renderer_drop = (renderer) => drop(renderer);
  env.renderer_frame = (renderer, width, height) => rendererFrame(deref(renderer), width, height);
  env.renderer_batch = (renderer, vertexPtr, vertexLen, indexPtr, indexLen, width, height) => {
    renderer = deref(renderer);
    const vertex = sliceF32FromWasm(vertexPtr, vertexLen);
    const index = sliceU16FromWasm(indexPtr, indexLen);
    rendererBatch(renderer, vertex, index, width, height);
  };

  const response = await fetch(playground_wasm);
  const { module, instance } = await WebAssembly.instantiateStreaming(response, imports);
  playground = instance.exports;
}

export function with_game(fn) {
  fn = alloc(fn);
  try {
    playground.with_game(fn);
  } finally {
    drop(fn);
  }
}

export function end(state) {
  playground.end(state);
}

export class Game {
  constructor(arena, game) {
    this.arena = arena;
    this.game = game;
  }

  drop() {
    this.game = undefined;
    this.arena = undefined;
  }

  run() {
    return playground.game_run(this.game, this.arena, canvasRef);
  }

  alloc(size, align) { return playground.arena_alloc(this.arena, size, align); }

  read_project(data) {
    const dataLen = data.byteLength;
    const dataPtr = this.alloc(dataLen, 1);
    sliceU8FromWasm(dataPtr, dataLen).set(data);

    playground.game_read_project(this.game, this.arena, dataPtr, dataLen);
  }

  visit(visitor) {
    visitor = alloc(visitor);
    try {
      playground.game_visit(this.game, visitor);
    } finally {
      drop(visitor);
    }
  }

  sprite(sprite, name) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    playground.game_sprite(this.game, sprite, namePtr, nameLen);
  }

  object(object, name) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    playground.game_object(this.game, object, namePtr, nameLen);
  }

  event(object, type, kind, code) {
    code = new TextEncoder().encode(code);
    const codeLen = code.byteLength;
    const codePtr = this.alloc(codeLen, 1);
    sliceU8FromWasm(codePtr, codeLen).set(code);

    playground.game_object_event(this.game, object, type, kind, codePtr, codeLen);
  }

  room(room, name) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    playground.game_room(this.game, room, namePtr, nameLen);
  }
}

const heap = [undefined, null, true, false];
let heapNext = heap.length;

function deref(idx) { return heap[idx]; }

function alloc(obj) {
  if (heapNext == heap.length) {
    heap.push(heap.length + 1);
  }
  const idx = heapNext;
  heapNext = heap[idx];
  heap[idx] = obj;
  return idx;
}

function drop(idx) {
  heap[idx] = heapNext;
  heapNext = idx;
}

function sliceU8FromWasm(ptr, len) {
  return new Uint8Array(playground.memory.buffer, ptr, len);
}
function sliceU16FromWasm(ptr, len) {
  return new Uint16Array(playground.memory.buffer, ptr, len);
}
function sliceF32FromWasm(ptr, len) {
  return new Float32Array(playground.memory.buffer, ptr, len);
}

const textDecoder = new TextDecoder();
function stringFromWasm(ptr, len) {
  return textDecoder.decode(sliceU8FromWasm(ptr, len));
}
