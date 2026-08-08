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
| `--filter <FILTER>` | `nearest` \| `bilinear` \| `bicubic` \| `xbrz` | `nearest` | Pixel scaling filter for display output. |
| `--remap <GUEST_KEY:KEY>` | `GUEST_KEY:KEY` | — | Remap a guest key to a host key. Repeatable. |
| `--show-gamepad` | flag | off | Draw a virtual gamepad overlay over the game frame. |
| `--fullscreen` | flag | off | Run in borderless fullscreen. |
| `--volume <VOLUME>` | 0–100 | `100` | Audio volume. |
| `--headless` | flag | off | Run without opening a window. |
| `--frames <COUNT>` | integer | `60` | Frames to run in headless mode. |
| `--repeat-delay <COUNT>` | integer | `10` | Frames a held key waits before auto-repeat starts. |
| `--repeat-period <COUNT>` | integer | `15` | Frames between auto-repeat pulses once repeating. |
| `--instruction-limit <COUNT>` | integer | — | Maximum guest instructions per callback. |
| `-S, --screenshot <PATH>` | path | — | Run headlessly, save a PNG screenshot, and exit. |
| `--screenshot-frames <COUNT>` | integer | `30` | Frames to run before capture. |
| `--save-state <PATH>` | path | — | Write a save state to this path when the emulator exits. |
| `--load-state <PATH>` | path | — | Load a save state from this path before running. |
| `-V, --version` | flag | — | Print the emulator version and exit. |
| `-v, --verbose` | flag | off | Enable debug logging. |

## Loading Games

The standalone emulator accepts `.CBE` files:

```bash
# Load a game directly
nicaiemu --file path/to/game.CBE

# Load with custom window size
nicaiemu --file path/to/game.CBE --width 600 --height 1000

# Run fullscreen with the xbrz pixel-art scaler
nicaiemu --file path/to/game.CBE --fullscreen --filter xbrz

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

The window also accepts mouse clicks as touch input on the 240×400 screen,
which touch-driven games (such as 魔塔) use for menus and in-game controls.

The `--remap` option rebinds a guest key to any host key using
`GUEST_KEY:KEY` syntax. Guest key names are `0`–`9`, `q`, `e`, `enter`,
`left`, `right`, `up`, `down`, `n`, and `m`; host keys use names such as
`space`, `x`, `f1`, or `backspace`. Escape is always reserved for exiting:

```bash
# Confirm with Space instead of Enter/F
nicaiemu --file path/to/game.CBE --remap enter:space

# Put the dpad on WASD and confirm on Space
nicaiemu --file path/to/game.CBE --remap up:w --remap down:s --remap left:a --remap right:d --remap enter:space
```

## Headless Mode

Run the emulator without a window — useful for automated testing and batch
processing:

```bash
# Run 120 frames silently and exit
nicaiemu --file path/to/game.CBE --headless --frames 120

# Older-style screenshot-only headless run
nicaiemu --file path/to/game.CBE --screenshot /dev/null --screenshot-frames 120
```

## Display Scaling

The guest framebuffer is 240×400. The window renders it centered with black
bars while preserving the aspect ratio, and the `--filter` option selects the
upscaler:

- `nearest` keeps hard pixel edges (best for pixel art);
- `bilinear` smooths pixels with 2×2 interpolation;
- `bicubic` applies separable Catmull-Rom interpolation;
- `xbrz` smooths pixel-art diagonals while retaining sharp edges.

`--show-gamepad` draws a virtual phone keypad over the game frame, highlighting
the currently held keys. The overlay is rendered at native resolution before
scaling, so it stays crisp at any window size.

## Audio

`--volume` sets the playback volume from 0 to 100. The emulator plays guest
WAV/MP3/MIDI audio at 44.1 kHz stereo through the default system output device.
If no device is available the emulator continues silently and logs a warning.

## Auto-Repeat

Held keys follow a feature-phone auto-repeat pattern: one visible step, a quiet
delay, then short pulses while the key stays held. `--repeat-delay` controls
the quiet delay and `--repeat-period` controls the distance between pulses:

```bash
# Faster walking: shorter delay and tighter pulses
nicaiemu --file path/to/game.CBE --repeat-delay 5 --repeat-period 8
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

# Fullscreen with the xbrz filter and gamepad overlay
nicaiemu --file path/to/game.CBE --fullscreen --filter xbrz --show-gamepad

# Take a screenshot and exit
nicaiemu --file path/to/game.CBE --screenshot shot.png --screenshot-frames 120

# Run and write a save state when the window closes
nicaiemu --file path/to/game.CBE --save-state game.sav

# Resume from a save state
nicaiemu --file path/to/game.CBE --load-state game.sav

# List archive contents
nicaiemu --file path/to/game.CBE --list

# Run 120 frames without a window
nicaiemu --file path/to/game.CBE --headless --frames 120

# Verbose logging
nicaiemu --file path/to/game.CBE --verbose
```
