# Risk of Rain 2 WASM autosplitter

A WIP Risk of Rain 2 Autosplitter/Load Remover using the livesplit-core
auto-splitting-v2 API. Adapted from the RiskOfRain2.asl. Rewrite in rust to use
the WIP ASR crate with mono runtime support.

## Building

Make sure you have the wasm32 target installed:
```sh
rustup target add wasm32-unknown-unknown
```

Then build using:
```sh
cargo build --release
```

## Usage

Place or link the autosplitter from `target/wasm32-unknown-unknown/ror2_autosplitter_rustasr.wasm` to any convenient location, then configure livesplit to use the wasm file.
Alternatively, download the pre-built wasm files from the release section.

## Current limitations:
 - Configuration support is currently very limited upstream, so recompilation is necesarry to change settings.
    - To change settings, open `src/lib.rs` and find the `struct AutoSplitterSettings` / `struct GameSettings` sections.
    - Change the default macro to the desired value eg.
```rust
    /// Split when leaving Bazaar Between Time
    #[default = true]
    bazaar: bool,
```
becomes
```rust
    /// Split when leaving Bazaar Between Time
    #[default = false]
    bazaar: bool,
```
to disable autosplitting when leaving Bazaar.
 - Uses workarounds for certain features that may already work upstream.

