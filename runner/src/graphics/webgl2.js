export function rendererNew(canvas, atlas, width, height) {
  const gl = canvas.getContext("webgl2");

  const program = buildProgram(gl, vs, fs);
  gl.useProgram(program);
  const viewLocation = 0;
  gl.uniformBlockBinding(program, gl.getUniformBlockIndex(program, "View"), viewLocation);
  const materialLocation = 1;
  gl.uniformBlockBinding(program, gl.getUniformBlockIndex(program, "Material"), materialLocation);
  const texLocation = 0;
  gl.uniform1i(gl.getUniformLocation(program, "tex"), texLocation);
  const positionLocation = gl.getAttribLocation(program, "vertex_position");
  const uvLocation = gl.getAttribLocation(program, "vertex_uv");
  const imageLocation = gl.getAttribLocation(program, "vertex_image");

  const view = gl.createBuffer();
  gl.bindBuffer(gl.UNIFORM_BUFFER, view);
  gl.bufferData(gl.UNIFORM_BUFFER, 32, gl.DYNAMIC_DRAW);

  const material = gl.createBuffer();
  gl.bindBuffer(gl.UNIFORM_BUFFER, material);
  gl.bufferData(gl.UNIFORM_BUFFER, 16, gl.DYNAMIC_DRAW);

  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texImage2D(
    gl.TEXTURE_2D, 0, gl.RGBA8,
    width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, atlas);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);

  const vbo = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.enableVertexAttribArray(positionLocation);
  gl.vertexAttribPointer(positionLocation, 3, gl.FLOAT, false, 36, 0);
  gl.enableVertexAttribArray(uvLocation);
  gl.vertexAttribPointer(uvLocation, 2, gl.FLOAT, false, 36, 12);
  gl.enableVertexAttribArray(imageLocation);
  gl.vertexAttribPointer(imageLocation, 4, gl.FLOAT, false, 36, 20);

  const ebo = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ebo);

  return {
    gl, program, viewLocation, materialLocation, texLocation,
    view, material, texture, vao, vbo, ebo
  };
}

function buildProgram(gl, vertex, fragment) {
  const program = gl.createProgram();
  gl.attachShader(program, buildShader(gl, gl.VERTEX_SHADER, vertex));
  gl.attachShader(program, buildShader(gl, gl.FRAGMENT_SHADER, fragment));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error(gl.getProgramInfoLog(program));
  }
  return program;
}

function buildShader(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.error(gl.getShaderInfoLog(shader));
  }
  return shader;
}

export function rendererFrame({ gl, program, viewLocation, view, vao }, roomWidth, roomHeight) {
  gl.canvas.style.width = `${roomWidth}px`;
  gl.canvas.style.height = `${roomHeight}px`;
  const width = window.devicePixelRatio * gl.canvas.clientWidth;
  const height = window.devicePixelRatio * gl.canvas.clientHeight;
  if (gl.canvas.width != width || gl.canvas.height != height) {
    gl.canvas.width = width;
    gl.canvas.height = height;
  }

  const gray = 192.0 / 255.0;
  gl.clearColor(gray, gray, gray, 1.0);
  gl.clear(gl.COLOR_BUFFER_BIT);

  const scale = Math.ceil(window.devicePixelRatio);
  gl.viewport(0, 0, width, height);
  gl.bindBuffer(gl.UNIFORM_BUFFER, view);
  gl.bufferSubData(gl.UNIFORM_BUFFER, 0, new Float32Array([
    width / scale, height / scale,
    width, height,
  ]));

  gl.bindBufferBase(gl.UNIFORM_BUFFER, viewLocation, view);
  gl.bindVertexArray(vao);
  gl.useProgram(program);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
}

export function rendererBatch({
  gl, materialLocation, texLocation, material, texture, vbo, ebo
}, vertex, index, width, height) {
  gl.bindBuffer(gl.UNIFORM_BUFFER, material);
  gl.bufferSubData(gl.UNIFORM_BUFFER, 0, new Float32Array([ width, height ]));

  gl.bindBufferBase(gl.UNIFORM_BUFFER, materialLocation, material);

  gl.activeTexture(gl.TEXTURE0 + texLocation);
  gl.bindTexture(gl.TEXTURE_2D, texture);

  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.bufferData(gl.ARRAY_BUFFER, vertex, gl.STREAM_DRAW);

  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ebo);
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, index, gl.STREAM_DRAW);

  gl.drawElements(gl.TRIANGLES, index.length, gl.UNSIGNED_SHORT, 0);
}

const vs = `#version 300 es

layout(std140) uniform View {
  vec2 view_size;
  vec2 port_size;
};

in vec3 vertex_position;
in vec2 vertex_uv;
in vec4 vertex_image;

out vec2 uv;
out vec4 image;

void main() {
  gl_Position = vec4(
    vertex_position.x * 2.0 / view_size.x - 1.0,
    vertex_position.y * -2.0 / view_size.y + 1.0,
    vertex_position.z,
    1.0);
  uv = vertex_uv;
  image = vertex_image;

  // D3D10 and later sample the viewport at pixel centers, while GM uses D3D8
  // which samples the viewport at the upper-left corners of pixels. Emulate
  // the older behavior by offsetting clip space by a half pixel.
  gl_Position.xy += vec2(1.0, -1.0) / port_size * gl_Position.w;
}`;

const fs = `#version 300 es
precision highp float;

layout(std140) uniform Material {
  vec2 atlas_size;
};

uniform sampler2D tex;

in vec2 uv;
in vec4 image;
flat in uint wrap;

out vec4 color;

void main() {
  // Emulate clamped texture sampling within the atlas texture.
  vec2 wh = image.zw;
  vec2 st = clamp(uv * wh, vec2(0.5, 0.5), wh - vec2(0.5, 0.5));
  vec2 uv = (image.xy + st) / atlas_size;

  color = texture(tex, uv).bgra;
}`;
