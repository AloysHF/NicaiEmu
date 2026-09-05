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
- Automatic landscape rotation: games packaged for the original phone's
  rotated landscape LCD are presented at 400×240 (the same content-identity
  profile the standalone frontend uses), with pointer taps mapped back to
  guest coordinates
- RetroPad input handling
- `.CBE` content loading
- Reset support that rebuilds the emulator state
- Input descriptors for frontend button labeling
- Save states with content-identity and checksum validation
- Guest memory exposure: the heap is reported as system RAM and the screen
  framebuffer as video RAM
- Audio output at 44.1 kHz stereo through the sample batch callback
- Touch and pointer input through the RetroArch pointer device, which covers
  both mouse and touchscreen input
- Core options for volume, touch input, auto BGM, and debug logging, applied
  live while a game is running
- Platform-accurate 10 Hz guest screen timing with continuous held-key state

## Core Options

Open **Settings > Core Options** in RetroArch to configure the core. Changes
apply immediately while a game is running and survive resets.

| Option | Choices (default first) | Description |
|---|---|---|
| Audio Volume (%) | 100 to 0 in steps of 10 | Master playback volume |
| Touch/Pointer Input | enabled, disabled | Whether mouse/touchscreen taps reach the guest |
| CPU/HLE Debug Logging | disabled, enabled | Forward debug-level core logs to the frontend log |
| Auto BGM (packaged MIDI) | disabled, enabled | Play the first packaged MIDI resource when the game never calls the audio manager |
| Display Rotation | auto, none, cw, ccw | Override the automatic landscape rotation (see below) |

## Screen Rotation

Games packaged for the original phone's rotated landscape LCD are presented
at 400×240 automatically through a built-in content profile, matching the
standalone frontend. For a landscape game outside the profile (sideways
picture), set **Display Rotation** to `cw` or `ccw` in the core options.

Prefer the core option over RetroArch's **Settings > Video > Rotation**
override: the core rotates the pixels itself, so a frontend rotation stacks
on top of it and turns profiled games sideways again. Pointer input is mapped
back through the active rotation, so taps stay correct in every mode.

## RetroPad Button Mapping

| RetroPad Button | Action |
|---|---|
| D-Pad Up | Up |
| D-Pad Down | Down |
| D-Pad Left | Left |
| D-Pad Right | Right |
| A (SNES East) | Confirm |
| B (SNES South) | Confirm |
| Start | Confirm |
| Select | — |
| X (SNES North) | Left soft key |
| Y (SNES West) | Right soft key |

## Current Limitations

- File-based MP3 control is not implemented yet
- Compatibility is limited to a subset of CBE applications
