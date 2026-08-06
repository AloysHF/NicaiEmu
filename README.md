# NicaiEmu

NicaiEmu is a Rust emulator for ARM/Thumb CBE applications used by Nicai/MStar mobile phones. It loads the executable and packaged resources directly, runs guest code through a pure-Rust ARM core, and bridges the phone services needed by supported games.

## Current capabilities

- CBE executable header, segment, checksum, section, and resource parsing
- ARM and Thumb guest execution, including Thumb BLX instructions used by CBE applications
- Firmware-style service tables for memory, resource, display, input, data streams, and formatted strings
- CBE RGB565 GIF reconstruction and clipped image drawing
- GBK text decoding with an embedded Unicode bitmap font
- 240×400 RGB565 framebuffer and a resizable desktop window
- Scriptable headless execution for deterministic compatibility testing
- SCE, MAP, and actor resource inspection helpers

The desktop frontend runs compatible native CBE executables. Libretro integration, audio output, save states, and broad compatibility across every CBE engine revision remain incomplete.

## Building

Install a current stable Rust toolchain, then build the desktop frontend:

```bash
cargo build --release -p nicaiemu
```

All runtime dependencies are implemented in Rust. Linux desktop builds also require the system libraries used by `minifb`.

## Running a game

```bash
cargo run --release -p nicaiemu -- --file path/to/game.CBE
```

Useful options:

```text
-f, --file <FILE>                 CBE executable to load
-l, --list                        List packaged resources and exit
-w, --width <WIDTH>               Initial window width (default: 480)
-H, --height <HEIGHT>             Initial window height (default: 800)
    --instruction-limit <COUNT>   Maximum guest instructions per callback
-S, --screenshot <PATH>           Run headlessly, save a PNG screenshot, and exit
    --screenshot-frames <COUNT>   Frames to run before capture (default: 30)
-v, --verbose                     Enable debug logging
```

Capture a screenshot after 120 frames without opening a window:

```bash
cargo run --release -p nicaiemu -- --file path/to/game.CBE \
  --screenshot frame.png --screenshot-frames 120
```

Controls:

| Phone input | Keyboard |
| --- | --- |
| Direction pad | Arrow keys or WASD |
| Confirm | Enter or F |
| Left/right soft keys | Q / E |
| Numeric keypad | 0–9 |
| Additional keys | N / M |
| Exit | Escape |

## Headless validation

The `cbe_boot` tool runs the same machine core without opening a window. A key event uses `FRAME:PHONE_KEY` syntax.

```bash
cargo run --release -p nicaiemu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 120 --key-event 1:14 --screenshot frame.png
```

Set `CBE_TRACE=all` to trace every bridged service, or provide comma-separated service filters such as `CBE_TRACE=4:24,6:3`. Tracing is disabled by default.

To capture every CBE application in the local validation directory, run:

```powershell
pwsh scripts/batch-screenshots.ps1
```

The script builds the standalone `nicaiemu` executable, runs each application
for 120 frames, and writes available PNG captures to `docs/images`. Use
`-Frames`, `-Binary`, `-GameDirectory`, or `-OutputDirectory` to override the
script defaults.

## Architecture

The workspace separates the platform-independent machine from its frontends:

```text
crates/nicaiemu-core/      CBE parser, ARM machine, services, and rendering
crates/nicaiemu/           Desktop window, input, and frame loop
crates/nicaiemu-tools/     Archive analysis and headless diagnostics
crates/nicaiemu-libretro/  Libretro integration scaffold
```

See [Architecture](docs/architecture.md) and
[Game Compatibility](docs/Game-Compatibility.md) for implementation details,
current limitations, and the latest batch results.

## Testing

```bash
cargo test --workspace --release
```

Game files are not included. Supply legally obtained CBE applications separately.

## License

BSD-3-Clause
