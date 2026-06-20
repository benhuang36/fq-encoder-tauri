# FQEncoder (Tauri)

Cross-platform desktop build of [FQEncoder](https://github.com/benhuang36/fq-encoder) —
encode any text into the 7 letters `F U C K Y O u` and back, with a resident tray
icon and loop-safe automatic clipboard encode/decode. Runs on **macOS, Windows, and
Linux** via [Tauri 2](https://tauri.app) (Rust backend + web frontend), for a tiny
footprint (~a few MB, ~30–80 MB idle).

The wire format is **identical** to the macOS Swift app: the same password decodes
strings across both (enforced by golden-vector tests in `src-tauri/src/codec.rs`).

The UI is available in **English** and **正體中文** — auto-detected from the system
locale, switchable from the header (`EN` / `中`), and remembered between launches.

## How it works

Three reversible stages (see [`src-tauri/src/codec.rs`](src-tauri/src/codec.rs)):

1. **Keystream XOR** — bytes XOR'd with a SHA-256 counter-mode keystream derived from
   the password (`seed = SHA256("FQEncoder.v1:" + password)`).
2. **Bidirectional diffusion** — a forward + backward additive chain (mod 256, `O(n)`)
   so every output byte depends on every input byte.
3. **Base-7 packing** — each byte → 3 symbols from `F U C K Y O u` (`7³ = 343 ≥ 256`).

> Obfuscation, not encryption: keyed but unauthenticated; an embedded/default key is
> recoverable from the binary.

The clipboard monitor (toggle from the tray menu) polls every 0.5s and is loop-safe
via content comparison: it skips its own writes and anything already seen, and never
writes back an empty or unchanged result.

## Develop

Requires Rust (stable) and Node 18+.

```sh
npm install
npm run tauri dev      # run with hot reload
cargo test --manifest-path src-tauri/Cargo.toml --lib   # codec parity tests
```

## Build / bundle

```sh
npm run tauri build    # produces a per-OS installer in src-tauri/target/release/bundle
```

`cargo tauri build` only bundles for the OS it runs on; the GitHub Actions workflow in
[`.github/workflows/build.yml`](.github/workflows/build.yml) builds all three from a
matrix. On Linux, install `libwebkit2gtk-4.1-dev` (+ `libappindicator3-dev`,
`librsvg2-dev`, `patchelf`) first.

## Layout

```
src/                  web frontend (vanilla TS + Vite): gradient/frosted UI
src-tauri/src/
├── codec.rs          keyed 3-stage codec + golden-vector parity tests
└── lib.rs            commands, tray, store-backed password, clipboard monitor
```

> New to Tauri? See the annotated walkthrough of how the pieces fit together: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) ([繁體中文](docs/ARCHITECTURE.zh-Hant.md)).

## Related

- [**fq-encoder**](https://github.com/benhuang36/fq-encoder) — the original native macOS (SwiftUI) app.

## License

MIT — see [LICENSE](LICENSE).
