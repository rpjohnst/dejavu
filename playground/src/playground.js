import { clear, outPrint, errPrint } from "./page.js";
import { start, stop } from "../../runner/src/platform/web.js";
import { rendererNew, rendererFrame, rendererBatch } from "../../runner/src/graphics/webgl2.js";
import playground_wasm from "./playground.wasm";
let playground;

export default async function init(canvas, output) {
  const imports = {};
  const env = imports.env = {};

  env.load_call = (load, arena, game) => {
    load = deref(load);
    const builder = new Builder(arena, game);
    try {
      load(builder);
    } finally {
      builder.drop();
    }
  };

  canvas = alloc(canvas);
  env.canvas = () => canvas;

  env.clear = () => clear(output);
  env.out_print = (ptr, len) => outPrint(output, stringFromWasm(ptr, len));
  env.err_print = (ptr, len) => errPrint(output, stringFromWasm(ptr, len));

  env.start = (fn, cx) => start(() => playground.__indirect_function_table.get(fn)(cx));
  env.stop = stop;

  env.renderer_new = (canvas, atlasPtr, atlasLen, width, height) => {
    canvas = deref(canvas);
    const atlas = sliceU8FromWasm(atlasPtr, atlasLen);
    const renderer = rendererNew(canvas, atlas, width, height);
    return alloc(renderer);
  };
  env.renderer_drop = (renderer) => drop(renderer);
  env.renderer_frame = (renderer) => rendererFrame(deref(renderer));
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

export function run(load) {
  load = alloc(load);
  try {
    playground.run(load);
  } finally {
    drop(load);
  }
}

export { stop };

export class Builder {
  constructor(arena, game) {
    this.arena = arena;
    this.game = game;
  }

  drop() {
    this.game = undefined;
    this.arena = undefined;
  }

  alloc(size, align) { return playground.arena_alloc(this.arena, size, align); }

  sprite(name, [ originX, originY ], [ sizeX, sizeY ], data) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    const dataLen = data.byteLength;
    const dataPtr = this.alloc(dataLen, 1);
    sliceU8FromWasm(dataPtr, dataLen).set(data);

    playground.game_sprite(
      this.game, namePtr, nameLen, originX, originY, sizeX, sizeY, dataPtr, dataLen);
  }

  object(name, sprite, events) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    playground.game_object(this.game, namePtr, nameLen, sprite, events);
  }

  event(type, kind, code) {
    code = new TextEncoder().encode(code);
    const codeLen = code.byteLength;
    const codePtr = this.alloc(codeLen, 1);
    sliceU8FromWasm(codePtr, codeLen).set(code);

    playground.game_object_event(this.game, type, kind, codePtr, codeLen);
  }

  room(name, objectIndex) {
    name = new TextEncoder().encode(name);
    const nameLen = name.byteLength;
    const namePtr = this.alloc(nameLen, 1);
    sliceU8FromWasm(namePtr, nameLen).set(name);

    playground.game_room(this.game, namePtr, nameLen, objectIndex);
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
