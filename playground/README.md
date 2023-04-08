# Playground

The playground is a web-based interface to experiment with Dejavu's GML interpreter.

It is currently hosted here: https://dejavu.abubalay.com/

# Building

In addition to the Rust toolchain needed to build Dejavu itself, the playground relies on [Binaryen] (optionally) and [Node.js].

[binaryen]: https://github.com/WebAssembly/binaryen/
[node.js]: https://nodejs.org/

To build the playground locally, run these commands from this directory:

```bash
# Build the playground crate to WebAssembly:
# (The wasm-opt step can be replaced with a simple file copy.)
rustup target add wasm32-unknown-unknown
cargo build --release
wasm-opt -Os -o src/playground.wasm ../target/wasm32-unknown-unknown/release/playground.wasm

# Build the user interface:
npm install
npm run build
```

During development, launch a live-reloading server on http://localhost:10001/ with this command instead of `npm run build`:

```bash
npm run start
```

(Changes to the Rust code will only be picked up after re-running `cargo build` and `wasm-opt`.)
