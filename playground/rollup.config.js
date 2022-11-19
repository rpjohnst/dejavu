import { nodeResolve } from "@rollup/plugin-node-resolve";
import url from "@rollup/plugin-url";
import terser from "@rollup/plugin-terser";
import livereload from "rollup-plugin-livereload";
import serve from "rollup-plugin-serve";

const release = !process.env.ROLLUP_WATCH;

export default {
  input: {
    bundle: "src/index.js"
  },
  output: {
    dir: "public/module",
    format: "es",
    sourcemap: !release,
  },
  plugins: [
    nodeResolve(),
    url({ limit: 0, include: "**/*.wasm", publicPath: "module/" }),
    release && terser(),
    !release && livereload("public"),
    !release && serve("public"),
  ],
  watch: {
    clearScreen: false
  }
};
