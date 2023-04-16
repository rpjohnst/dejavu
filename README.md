# Dejavu

Dejavu is a free, open source implementation of [Game Maker]. It is designed to be compatible with classic versions (8.0 and earlier) and eventually more recent versions (Game Maker: Studio). The goal is to preserve some of indie game dev history, and port it to modern platforms.

You can [**try it out**][playground] in your web browser right now!

[game maker]: https://en.wikipedia.org/wiki/GameMaker_Studio
[playground]: https://dejavu.abubalay.com/

# Building

Dejavu currently requires a nightly Rust toolchain, though mostly out of convenience. It should move to stable Rust in the future. When Rust is installed using `rustup`, the `rust-toolchain.toml` file will ensure a nightly toolchain is installed and select it automatically.

With Rust installed, build and run using `cargo`:

```bash
cargo run -- project.gmk
```

This command accepts Game Maker 8.0 project files (`project.gmk`), executables (`game.exe`), and stand-alone scripts (`script.gml`).

See also the build instructions for the [playground](playground).

# Contributing

Dejavu is still in an early stage, but contributions are welcome in the form of bug reports, pull requests, suggestions, and other feedback.

In its current state, it can compile and run Game Maker Language (GML) and drag-and-drop code, with some support for binding to APIs implemented in Rust. Pick a game and try running it to see what we need to implement next!

There is a [Discord server] for contributors and users to coordinate and discuss. Stop by and chat!

[discord server]: https://discord.gg/5VCBZwj

# License

Dejavu is distributed under the terms of both the [MIT license] and the [Apache license]. Pull requests are accepted under the same terms.

[mit license]: LICENSE-MIT
[apache license]: LICENSE-APACHE
