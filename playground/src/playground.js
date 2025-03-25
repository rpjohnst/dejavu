import { clear, outPrint, errPrint } from "./page.js";
import { schedule, cancel } from "../../runner/src/platform/web.js";
import { rendererNew, rendererFrame, rendererBatch } from "../../runner/src/graphics/webgl2.js";
import playground_wasm from "./playground.wasm";
let playground, gameLayout;

let canvasRef;

export default async function init(canvas, output) {
  const imports = {};
  const env = imports.env = {};

  env.call_ptr = (fn, ptr0) => deref(fn)(ptr0);

  canvasRef = alloc(canvas);

  env.clear = () => clear(output);
  env.out_print = (ptr, len) => outPrint(output, stringFromWasm(ptr, len));
  env.err_print = (ptr, len) => errPrint(output, stringFromWasm(ptr, len));

  env.schedule = (fn, cx) => schedule(timestamp => {
    playground.__indirect_function_table.get(fn)(cx, timestamp);
  });
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

  const view = new DataView(playground.memory.buffer);
  gameLayout = loadLayout(view, playground.GAME_LAYOUT.value);
}

export function readProject(data) {
  let project;
  withArena(arena => {
    const dataLen = data.byteLength;
    const dataPtr = playground.arena_alloc(arena, dataLen, 1);
    sliceU8FromWasm(dataPtr, dataLen).set(data);
    withGame(game => {
      playground.read_project(game, arena, dataPtr, dataLen);

      const view = new DataView(playground.memory.buffer);
      project = loadValue(view, game, gameLayout);
    });
  });
  return project;
}

export function run(project) {
  let state;
  withArena(arena => {
    withGame(game => {
      const view = new DataView(playground.memory.buffer);
      storeValue(view, game, gameLayout, arena, project);

      state = playground.run(game, arena, canvasRef);
    });
  });
  return state;
}

export function end(state) {
  playground.end(state);
}

function withArena(fn) {
  fn = alloc(fn);
  try {
    return playground.with_arena(fn);
  } finally {
    drop(fn);
  }
}

function withGame(fn) {
  fn = alloc(fn);
  try {
    return playground.with_game(fn);
  } finally {
    drop(fn);
  }
}

function loadLayout(view, data) {
  switch (view.getUint8(data + 0)) {
  case 0: return {
    kind: "bool"
  };
  case 1: return {
    kind: "integer", signed: view.getUint8(data + 1) != 0, size: view.getUint32(data + 4, true)
  };
  case 2: return {
    kind: "float", size: view.getUint32(data + 4, true)
  };
  case 3: return {
    kind: "array",
    item: loadLayout(view, view.getUint32(data + 4, true)),
    stride: view.getUint32(data + 8, true),
    len: view.getUint32(data + 12, true),
  };
  case 4: {
    const fields = {};
    const ptr = playground.fields_ptr(data + 4), len = playground.fields_len(data + 4);
    for (let data = ptr; data < ptr + 16 * len; data += 16) {
      const name = stringFromWasm(playground.bstr_ptr(data + 0), playground.bstr_len(data + 0));
      fields[name] = {
        offset: view.getUint32(data + 8, true),
        layout: loadLayout(view, view.getUint32(data + 12, true))
      };
    }
    return { kind: "struct", fields };
  }
  case 5: return { kind: "bstr" };
  case 6: return {
    kind: "slice",
    item: loadLayout(view, view.getUint32(data + 4, true)),
    stride: view.getUint32(data + 8, true),
    ptr: playground.__indirect_function_table.get(view.getUint32(data + 12, true)),
    len: playground.__indirect_function_table.get(view.getUint32(data + 16, true)),
    store: playground.__indirect_function_table.get(view.getUint32(data + 20, true)),
  };
  case 7: return {
    kind: "vec",
    item: loadLayout(view, view.getUint32(data + 4, true)),
    stride: view.getUint32(data + 8, true),
    ptr: playground.__indirect_function_table.get(view.getUint32(data + 12, true)),
    len: playground.__indirect_function_table.get(view.getUint32(data + 16, true)),
    resize: playground.__indirect_function_table.get(view.getUint32(data + 20, true)),
  };
  default: throw new Error("Unexpected layout kind");
  }
}

function loadValue(view, data, layout) {
  switch (layout.kind) {
  case "bool":
    return view.getUint8(data) != 0;
  case "integer":
    if (layout.signed) {
      switch (layout.size) {
      case 1: return view.getInt8(data);
      case 2: return view.getInt16(data, true);
      case 4: return view.getInt32(data, true);
      case 8: return view.getBigInt64(data, true);
      default: throw new Error("Unexpected integer size");
      }
    } else {
      switch (layout.size) {
      case 1: return view.getUint8(data);
      case 2: return view.getUint16(data, true);
      case 4: return view.getUint32(data, true);
      case 8: return view.getBigUint64(data, true);
      default: throw new Error("Unexpected integer size");
      }
    }
  case "float":
    switch (layout.size) {
    case 4: return view.getFloat32(data, true);
    case 8: return view.getFloat64(data, true);
    default: throw new Error("Unexpected float size");
    }
  case "array":
    return loadArray(view, layout, data, layout.len);
  case "struct": {
    const struct = {};
    for (const name in layout.fields) {
      const field = layout.fields[name];
      struct[name] = loadValue(view, data + field.offset, field.layout);
    }
    return struct;
  }
  case "bstr": {
    return stringFromWasm(playground.bstr_ptr(data), playground.bstr_len(data));
  }
  case "slice": case "vec":
    return loadArray(view, layout, layout.ptr(data), layout.len(data));
  default: throw new Error("Unexpected layout kind");
  }
}

function loadArray(view, layout, ptr, len) {
  switch (layout.item.kind) {
  case "integer":
    if (layout.item.signed) {
      switch (layout.item.size) {
      case 1: return new sliceI8FromWasm(ptr, len).slice();
      case 2: return new sliceI16FromWasm(ptr, len).slice();
      case 4: return new sliceI32FromWasm(ptr, len).slice();
      }
    } else {
      switch (layout.item.size) {
      case 1: return new sliceU8FromWasm(ptr, len).slice();
      case 2: return new sliceU16FromWasm(ptr, len).slice();
      case 4: return new sliceU32FromWasm(ptr, len).slice();
      }
    }
  default:
    const array = [];
    for (let data = ptr; data < ptr + layout.stride * len; data += layout.stride) {
      array.push(loadValue(view, data, layout.item));
    }
    return array;
  }
}

function storeValue(view, data, layout, arena, value) {
  switch (layout.kind) {
  case "bool":
    return view.setUint8(data, value != 0);
  case "integer":
    if (layout.signed) {
      switch (layout.size) {
      case 1: return view.setInt8(data, value);
      case 2: return view.setInt16(data, value, true);
      case 4: return view.setInt32(data, value, true);
      case 8: return view.setBigInt64(data, value, true);
      default: throw new Error("Unexpected integer size");
      }
    } else {
      switch (layout.size) {
      case 1: return view.setUint8(data, value);
      case 2: return view.setUint16(data, value, true);
      case 4: return view.setUint32(data, value, true);
      case 8: return view.setBigUint64(data, value, true);
      default: throw new Error("Unexpected integer size");
      }
    }
  case "float":
    switch (layout.size) {
    case 4: return view.setFloat32(data, value, true);
    case 8: return view.setFloat64(data, value, true);
    default: throw new Error("Unexpected float size");
    }
  case "array":
    return storeArray(view, data, layout.len, layout, arena, value);
  case "struct": {
    for (const name in layout.fields) {
      const field = layout.fields[name];
      storeValue(view, data + field.offset, field.layout, arena, value[name]);
    }
    return;
  }
  case "bstr": {
    const str = new TextEncoder().encode(value);
    const len = str.byteLength;
    const ptr = playground.arena_alloc(arena, len, 1);
    playground.bstr_store(data, ptr, len);
    return sliceU8FromWasm(ptr, len).set(str);
  }
  case "slice": {
    const len = value.length;
    const ptr = playground.arena_alloc(arena, layout.stride * len, layout.stride);
    layout.store(data, ptr, len);
    return storeArray(view, ptr, len, layout, arena, value);
  }
  case "vec": {
    const len = value.length;
    const ptr = layout.resize(data, len);
    return storeArray(view, ptr, len, layout, arena, value);
  }
  default: throw new Error("Unexpected layout kind");
  }
}

function storeArray(view, ptr, len, layout, arena, value) {
  switch (layout.item.kind) {
  case "integer": {
    if (layout.item.signed) {
      switch (layout.item.size) {
      case 1: return sliceI8FromWasm(ptr, len).set(value);
      case 2: return sliceI16FromWasm(ptr, len).set(value);
      case 4: return sliceI32FromWasm(ptr, len).set(value);
      }
    } else {
      switch (layout.item.size) {
      case 1: return sliceU8FromWasm(ptr, len).set(value);
      case 2: return sliceU16FromWasm(ptr, len).set(value);
      case 4: return sliceU32FromWasm(ptr, len).set(value);
      }
    }
  }
  default: {
    for (let i = 0; i < value.length; i++) {
      storeValue(view, ptr + layout.stride * i, layout.item, arena, value[i]);
    }
    return;
  }
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
function sliceU32FromWasm(ptr, len) {
  return new Uint32Array(playground.memory.buffer, ptr, len);
}
function sliceI8FromWasm(ptr, len) {
  return new Int8Array(playground.memory.buffer, ptr, len);
}
function sliceI16FromWasm(ptr, len) {
  return new Int16Array(playground.memory.buffer, ptr, len);
}
function sliceI32FromWasm(ptr, len) {
  return new Int32Array(playground.memory.buffer, ptr, len);
}
function sliceF32FromWasm(ptr, len) {
  return new Float32Array(playground.memory.buffer, ptr, len);
}

const textDecoder = new TextDecoder();
function stringFromWasm(ptr, len) {
  return textDecoder.decode(sliceU8FromWasm(ptr, len));
}
