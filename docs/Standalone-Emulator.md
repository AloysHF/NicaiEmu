# Standalone Emulator

This guide covers installing and running the standalone `nicaiemu` binary,
loading games, keyboard controls, headless mode, and all command-line options.

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | ✅ |
| macOS | x86_64, aarch64 | ✅ |
| Linux | x86_64, aarch64 | ✅ |

## Installation

Download the latest standalone binary for your platform from the
[Releases](https://github.com/jiangxincode/NicaiEmu/releases) page.

You can also build it from source:

```bash
cargo build -p nicaiemu --release
```

The binary is produced at `target/release/nicaiemu` (`.exe` on Windows).

## Synopsis

```text
nicaiemu [OPTIONS] --file <GAME_PATH>
```

## Options

| Option | Value | Default | Description |
|---|---|---|---|
| `-f, --file <FILE>` | path | *required* | Path to the CBE game file. |
| `-l, --list` | flag | off | List packaged resources and exit. |
| `-w, --width <WIDTH>` | integer | `480` | Initial window width. |
| `-H, --height <HEIGHT>` | integer | `800` | Initial window height. |
| `--instruction-limit <COUNT>` | integer | — | Maximum guest instructions per callback. |
| `-S, --screenshot <PATH>` | path | — | Run headlessly, save a PNG screenshot, and exit. |
| `--screenshot-frames <COUNT>` | integer | `30` | Frames to run before capture. |
| `--save-state <PATH>` | path | — | Write a save state to this path when the emulator exits. |
| `--load-state <PATH>` | path | — | Load a save state from this path before running. |
| `-v, --verbose` | flag | off | Enable debug logging. |

## Loading Games

The standalone emulator accepts `.CBE` files:

```bash
# Load a game directly
nicaiemu --file path/to/game.CBE

# Load with custom window size
nicaiemu --file path/to/game.CBE --width 600 --height 1000

# List resources in the archive
nicaiemu --file path/to/game.CBE --list
```

## Default Key Mappings

| Phone Input | Keyboard |
| --- | --- |
| Direction pad | Arrow keys or WASD |
| Confirm | Enter or F |
| Left soft key | Q |
| Right soft key | E |
| Numeric keypad | 0–9 |
| Additional keys | N / M |
| Reset | R |
| Exit | Escape |

## Headless Mode

Run the emulator without a window — useful for automated testing and batch
processing:

```bash
# Run 120 frames silently
nicaiemu --file path/to/game.CBE --screenshot /dev/null --screenshot-frames 120
```

## Screenshot Mode

Capture a PNG screenshot after a number of frames, then exit:

```bash
# Take a screenshot after 120 frames
nicaiemu --file path/to/game.CBE --screenshot screenshot.png --screenshot-frames 120
```

This is used by the batch screenshot script (`scripts/batch-screenshots.ps1`)
to generate screenshots for all games at once. Screenshots always use the
native framebuffer resolution.

## Headless Validation Tool

The `cbe_boot` tool runs the same machine core without opening a window. A key
event uses `FRAME:PHONE_KEY` syntax.

```bash
cargo run --release -p nicaiemu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 120 --key-event 1:14 --screenshot frame.png
```

Set `CBE_TRACE=all` to trace every bridged service, or provide comma-separated
service filters such as `CBE_TRACE=4:24,6:3`. Tracing is disabled by default.

## Batch Screenshots

To capture every CBE application in the local validation directory, run:

```powershell
pwsh scripts/batch-screenshots.ps1
```

The script builds the standalone `nicaiemu` executable, runs each application
for 120 frames, and writes PNG captures to `docs/images`. Headless capture keeps
the last guest-rendered frame if a later callback stops. Blank frames and
applications that stop before drawing are reported as failures. Use
`-Frames`, `-Binary`, `-GameDirectory`, or `-OutputDirectory` to override the
script defaults.

## Examples

```bash
# Basic usage
nicaiemu --file path/to/game.CBE

# Custom window size
nicaiemu --file path/to/game.CBE --width 600 --height 1000

# Take a screenshot and exit
nicaiemu --file path/to/game.CBE --screenshot shot.png --screenshot-frames 120

# Run and write a save state when the window closes
nicaiemu --file path/to/game.CBE --save-state game.sav

# Resume from a save state
nicaiemu --file path/to/game.CBE --load-state game.sav

# List archive contents
nicaiemu --file path/to/game.CBE --list

# Verbose logging
nicaiemu --file path/to/game.CBE --verbose
```

## Audio

The standalone emulator plays guest WAV/MP3 audio at 44.1 kHz stereo through
the default system output device. If no device is available the emulator
continues silently and logs a warning. Guest MIDI playback is not implemented
yet; games that submit MIDI data currently skip it and keep running.
