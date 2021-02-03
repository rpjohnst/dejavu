# Playground

The playground is a web-based interface to experiment with Dejavu's GML interpreter.

It is currently hosted here: https://dejavu.abubalay.com/

# Building

In addition to the Rust toolchain needed to build Dejavu itself, the playground relies on [wasm-pack] and [Node.js].

[wasm-pack]: https://rustwasm.github.io/wasm-pack/
[node.js]: https://nodejs.org/

To build the playground locally, run these commands from this directory:

```bash
# Build the playground crate to WebAssembly:
wasm-pack build --target web

# Build the user interface:
npm install
npm run build
```

Instead of `npm run build`, this command launches a live-reloading development server on http://localhost:10001/:

```bash
npm run start
```

(Changes to the Rust code will only be picked up after re-running `wasm-pack`.)
