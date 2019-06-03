import resolve from "rollup-plugin-node-resolve";
import commonjs from "rollup-plugin-commonjs";
import livereload from "rollup-plugin-livereload";
import serve from "rollup-plugin-serve";
import { terser } from "rollup-plugin-terser";
import url from "rollup-plugin-url";

const release = !process.env.ROLLUP_WATCH;

export default {
  input: {
    bundle: "src/index.js"
  },
  output: {
    dir: "public/module",
    format: "esm",
    sourcemap: true,
  },
  plugins: [
    resolve(),
    commonjs(),
    url({ limit: 0, include: "**/*.wasm", publicPath: "module/" }),
    release && terser(),
    !release && livereload("public"),
    !release && serve("public"),
  ],
  watch: {
    clearScreen: false
  }
};
