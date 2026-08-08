# RetroArch Core

This guide covers installing and running the NicaiEmu libretro core for
RetroArch, loading content, supported features, and controls.

## Supported Platforms

| Platform | Architecture | Standalone | Libretro |
|----------|-------------|------------|----------|
| Windows | x86_64 | ✅ | ✅ |
| macOS | x86_64, aarch64 | ✅ | ✅ |
| Linux | x86_64, aarch64 | ✅ | ✅ |

## Installation

### Manual Installation

Build the libretro core:

```bash
cargo build -p nicaiemu-libretro --release
```

Cargo names the cdylib after its lib target, so this produces
`nicaiemu.dll` on Windows (`libnicaiemu.so` on Linux,
`libnicaiemu.dylib` on macOS) under `target/release/`.

RetroArch expects the core file to be named `nicaiemu_libretro.<ext>`, so
rename it accordingly before placing it into RetroArch's `cores/` directory.
Copy `nicaiemu_libretro.info` into RetroArch's `info/` directory so the
frontend can display the core metadata and supported features.

## Loading Games

1. Open RetroArch and select **Load Core > Nicai/MStar CBE (NicaiEmu)**.
2. Select **Load Content**.
3. Choose a `.CBE` file.

## Supported Features

- Video output using the RGB888 pixel format
- RetroPad input handling
- `.CBE` content loading

## RetroPad Button Mapping

| RetroPad Button | Action |
|---|---|
| D-Pad Up | Up |
| D-Pad Down | Down |
| D-Pad Left | Left |
| D-Pad Right | Right |
| A (SNES East) | Confirm |
| B (SNES South) | Cancel |
| Start | — |
| Select | — |
| X (SNES North) | Left soft key |
| Y (SNES West) | Right soft key |

## Current Limitations

- Audio output is not implemented
- Save states are not implemented
- Core options are not yet available
- Compatibility is limited to a subset of CBE applications
