export function renderer_new(canvas, atlas, width, height) {
  const gl = canvas.getContext("webgl2");

  const program = buildProgram(gl, vs, fs);
  gl.useProgram(program);
  const viewLocation = 0;
  const viewIndex = gl.getUniformBlockIndex(program, "View");
  gl.uniformBlockBinding(program, viewIndex, viewLocation);
  const texLocation = 0;
  gl.uniform1i(gl.getUniformLocation(program, "tex"), texLocation);
  const posLocation = gl.getAttribLocation(program, "vertex_pos");
  const uvLocation = gl.getAttribLocation(program, "vertex_uv");

  const ubo = gl.createBuffer();
  gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
  const uboSize = gl.getActiveUniformBlockParameter(program, viewIndex, gl.UNIFORM_BLOCK_DATA_SIZE);
  if (uboSize == 0) {
    console.error("Invalid UNIFORM_BLOCK_DATA_SIZE: ", uboSize);
  }
  if (uboSize < 8) {
    uboSize = 8;
  }
  gl.bufferData(gl.UNIFORM_BUFFER, uboSize, gl.DYNAMIC_DRAW);

  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texImage2D(
    gl.TEXTURE_2D, 0, gl.SRGB8_ALPHA8,
    width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, atlas);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);

  const vbo = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.enableVertexAttribArray(posLocation);
  gl.vertexAttribPointer(posLocation, 3, gl.FLOAT, false, 20, 0);
  gl.enableVertexAttribArray(uvLocation);
  gl.vertexAttribPointer(uvLocation, 2, gl.FLOAT, false, 20, 12);

  const ebo = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ebo);

  return { gl, program, viewLocation, texLocation, ubo, texture, vao, vbo, ebo };
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

export function renderer_frame({ gl, program, viewLocation, ubo, vao }) {
  const width = window.devicePixelRatio * gl.canvas.clientWidth;
  const height = window.devicePixelRatio * gl.canvas.clientHeight;
  if (gl.canvas.width != width || gl.canvas.height != height) {
    gl.canvas.width = width;
    gl.canvas.height = height;
  }

  gl.clearColor(0.75, 0.75, 0.75, 1.0);
  gl.clear(gl.COLOR_BUFFER_BIT);

  const scale = Math.ceil(window.devicePixelRatio);
  gl.viewport(0, 0, width, height);
  gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
  gl.bufferSubData(gl.UNIFORM_BUFFER, 0, new Float32Array([width / scale, height / scale]));

  gl.bindBufferBase(gl.UNIFORM_BUFFER, viewLocation, ubo);
  gl.bindVertexArray(vao);
  gl.useProgram(program);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
}

export function renderer_batch({ gl, texLocation, texture, vbo, ebo }, vertex, index) {
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
};

in vec3 vertex_pos;
in vec2 vertex_uv;

out vec2 uv;

void main() {
  gl_Position = vec4(
    vertex_pos.x * 2.0 / view_size.x - 1.0,
    vertex_pos.y * -2.0 / view_size.y + 1.0,
    vertex_pos.z,
    1.0);
  uv = vertex_uv;
}`;

const fs = `#version 300 es
precision highp float;

uniform sampler2D tex;

in vec2 uv;

out vec4 color;

vec4 fromLinear(vec4 linear) {
  bvec3 threshold = lessThan(linear.rgb, vec3(0.0031308));
  vec3 above = vec3(1.055) * pow(linear.rgb, vec3(1.0 / 2.4)) - vec3(0.055);
  vec3 below = linear.rgb * vec3(12.92);
  return vec4(mix(above, below, threshold), linear.a);
}

void main() {
  color = fromLinear(texture(tex, uv));
}`;
