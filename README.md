# Risk of Rain 2 WASM autosplitter

A cross-platform Risk of Rain 2 Autosplitter/Load Remover.
Uses the livesplit-core auto-splitting-v2 API. Adapted from the RiskOfRain2.asl.
Rewritten in rust to use the ASR crate with mono runtime support.

Works with LSO and regular LiveSplit via the autosplitting runtime component.

The `fast-reset-detection` branch detects resets more quickly than the regular
branch. Runners can know when to reset before the first stage fully loads,
which is before the original timer starts, and that causes the reset to be
skipped. This branch works around the problem by starting the timer a little
earlier (identical to previous versions' start timing with the -0.56s offset),
and compensates by pausing at 0:00 until the start condition matches the
original timer's start condition. The downside to this method is that the
"Real Time" of the timer will include the paused time at the start.

Runners will need to manually subtract 0.56s from their "Real time" when
submitting runs using this branch. "Game Time" needs no adjustments.


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
 - Configuration support is still spotty in LSO, so recompilation may be necesarry for settings to persist.
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
 - Due to a limitation in the runtime, Game Time is not recorded for the first split, and the split time is not shown.

