# NicaiEmu — A Nicai/MStar CBE game emulator written in Rust

<p align="center">
  <img src="res/logo-banner.png" alt="NicaiEmu" width="600">
</p>

<p align="center">
  <a href="https://jiangxincode.github.io/NicaiEmu/"><img src="https://img.shields.io/badge/Website-NicaiEmu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/jiangxincode/NicaiEmu/actions/workflows/ci.yml"><img src="https://github.com/jiangxincode/NicaiEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/jiangxincode/NicaiEmu/releases/latest"><img src="https://img.shields.io/github/v/release/jiangxincode/NicaiEmu" alt="Release"></a>
  <a href="https://github.com/jiangxincode/NicaiEmu/releases"><img src="https://img.shields.io/github/downloads/jiangxincode/NicaiEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=jiangxincode_NicaiEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=jiangxincode_NicaiEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSD%203--Clause-blue.svg" alt="License: BSD 3-Clause"></a>
  <a href="https://discord.gg/7XDdSrYD"><img src="https://img.shields.io/badge/Discord-Join%20Us-5865F2?logo=discord&logoColor=white" alt="Discord"></a>
  <a href="https://qm.qq.com/q/LAO7DKAWUC"><img src="https://img.shields.io/badge/QQ%E7%BE%A4-Join%20Us-12B7F5?logo=tencent-qq&logoColor=white" alt="QQ Group"></a>
</p>

NicaiEmu is a Rust emulator for ARM/Thumb CBE applications used by Nicai/MStar
mobile phones. It loads the executable and packaged resources directly, runs
guest code through a pure-Rust ARM core, and bridges the phone services needed
by supported games.

## Features

- **CBE format support** — section parsing, resource lookup, image decoding
- **ARM/Thumb CPU emulation** — little- and big-endian execution, interworking branches
- **Service bridge** — firmware-style API for memory, resources, display, input, and text
- **Graphics rendering** — RGB565 framebuffer with GIF and PNG image reconstruction
- **Text rendering** — GBK decoding with embedded Unicode bitmap font
- **240×400 display** — native WQVGA resolution with resizable desktop window
- **Headless mode** — run without a window for testing and batch processing
- **Screenshot capture** — automated PNG screenshot generation
- **Save states** — versioned, checksummed snapshots of the full machine state
  through the libretro API and the standalone `--save-state` / `--load-state`
  options
- **Reset** — rebuilds the emulator runtime state from the loaded archive
  (standalone `R` key and libretro `retro_reset`)
- **Guest memory exposure** — libretro exposes the guest heap and screen
  framebuffer to frontend memory tools
- **Audio** — WAV/MP3 decoding, stereo mixing, volume control, and 44.1 kHz
  output through the libretro sample callback and the standalone device sink
  (guest MIDI synthesis and file-based MP3 control are planned follow-ups)
- **Libretro integration** — playable libretro core with RGB888 video output,
  RetroPad input, content loading, save states, reset, and memory exposure
  (core options are planned for a later milestone)

## Usage

### Standalone Mode

Download the latest binary from the
[Releases](https://github.com/jiangxincode/NicaiEmu/releases) page and run:

```bash
nicaiemu --file path/to/game.CBE
```

See the [Standalone Emulator](docs/Standalone-Emulator.md) guide for
installation, keyboard controls, headless mode, screenshots, and all
command-line options.

### RetroArch Mode

Install the core and load a game through RetroArch's **Load Content** menu.

See the [RetroArch Core](docs/RetroArch-Core.md) guide for installation,
supported platforms, RetroPad mapping, and features.

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode

```bash
cargo build -p nicaiemu --release
cargo run -p nicaiemu --release -- --file path/to/game.CBE
```

### Libretro Core (for RetroArch)

```bash
cargo build -p nicaiemu-libretro --release
```

The binary is produced at `target/release/nicaiemu.dll`
(`libnicaiemu.so` on Linux, `libnicaiemu.dylib` on macOS). Rename it to
`nicaiemu_libretro.<ext>` before placing it in RetroArch's `cores/`
directory.

For Android cross-compilation, see [Android Libretro Core](docs/Android-Libretro-Core.md).
For iOS, see [iOS Libretro Core](docs/iOS-Libretro-Core.md).

## Architecture

```
crates/
├── nicaiemu-core/         # Platform-independent emulator engine (library)
│   └── src/
│       ├── lib.rs            # Crate root
│       ├── machine.rs        # Core machine tying all components together
│       ├── arm_cpu.rs        # ARM/Thumb CPU emulation
│       ├── memory.rs         # Sparse memory regions
│       ├── cbe_archive.rs    # CBE container parser
│       ├── cbe_executable.rs # CBE executable loader
│       ├── renderer.rs       # RGB565 → RGB888 framebuffer conversion
│       ├── services.rs       # Firmware service bridge
│       ├── text.rs           # GBK text and font rendering
│       └── ...
├── nicaiemu/              # Standalone binary (→ nicaiemu)
│   └── src/
│       ├── main.rs           # Window loop, CLI, keyboard input
│       └── ...
├── nicaiemu-tools/        # Archive analysis and headless diagnostics
│   └── src/
│       ├── bin/
│       │   ├── cbe_boot.rs   # Headless boot tool
│       │   └── cbe_ls.rs     # Archive listing tool
│       └── ...
└── nicaiemu-libretro/     # Libretro cdylib (→ nicaiemu_libretro.{dll,so,dylib})
    ├── nicaiemu_libretro.info   # RetroArch core metadata
    └── src/
        ├── lib.rs               # cdylib crate root
        └── libretro/
            ├── api.rs           # Exported libretro functions
            ├── callbacks.rs     # Callback management
            ├── constants.rs     # libretro constants
            ├── logger.rs        # Bridges the `log` crate to the frontend
            └── types.rs         # libretro type definitions
```

See [Architecture](docs/architecture.md) for implementation details.

## Key Mappings (Standalone)

| Phone Input | Keyboard |
| --- | --- |
| Direction pad | Arrow keys or WASD |
| Confirm | Enter or F |
| Left soft key | Q |
| Right soft key | E |
| Numeric keypad | 0–9 |
| Additional keys | N / M |
| Exit | Escape |

Direction presses advance one step; holding a direction key auto-repeats so
the character keeps moving while the key stays down.

## Game Compatibility

The emulator supports CBE applications for Nicai/MStar phones with 240×400
display. 75 out of 75 tested games render startup frames successfully.

| Status | Count |
|--------|-------|
| ✅ Pass | 75 |
| ❌ Fail | 0 |

For the full game list with screenshots, see [Game Compatibility](docs/Game-Compatibility.md).

## Testing

Run the unit tests:

```bash
cargo test --workspace --release
```

Game files are not included. Supply legally obtained CBE applications separately.

## Contributing

Contributions are welcome! Whether you're interested in fixing bugs, adding
features, improving documentation, or testing game compatibility, we'd love your
help. See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for details.

## License

This project is licensed under the [BSD 3-Clause License](LICENSE).
