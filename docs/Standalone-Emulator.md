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
[Releases](https://github.com/AloysHF/NicaiEmu/releases) page.

You can also build it from source:

```bash
cargo build -p nicaiemu --release
```

The binary is produced at `target/release/nicaiemu` (`.exe` on Windows).

## Synopsis

```text
nicaiemu [OPTIONS] <GAME_PATH>
```

## Options

| Option | Value | Default | Description |
|---|---|---|---|
| `<GAME_PATH>` | path | *required* | Path to the CBE game file. |
| `-l, --list` | flag | off | List packaged resources and exit. |
| `-w, --width <WIDTH>` | integer | `480` | Initial window width. |
| `-H, --height <HEIGHT>` | integer | `800` | Initial window height. |
| `--filter <FILTER>` | `nearest` \| `bilinear` \| `bicubic` \| `xbrz` | `nearest` | Pixel scaling filter for display output. |
| `--rotate <ROTATION>` | `auto` \| `none` \| `cw` \| `ccw` | `auto` | Rotate the guest framebuffer before presentation. `auto` uses the built-in landscape-game profile; explicit values override it. |
| `--rotation-profile <FILE>` | path | — | Load extra display-rotation entries from a CSV file before starting (see [Screen rotation](#screen-rotation)). |
| `--remap <GUEST_KEY:KEY>` | `GUEST_KEY:KEY` | — | Remap a guest key to a host key. Repeatable. |
| `--show-gamepad` | flag | off | Draw a virtual gamepad overlay over the game frame. |
| `--fullscreen` | flag | off | Run in borderless fullscreen. |
| `--volume <VOLUME>` | 0–100 | `100` | Audio volume. |
| `--headless` | flag | off | Run without opening a window. |
| `--frames <COUNT>` | integer | `60` | Frames to run in headless mode. |
| `--instruction-limit <COUNT>` | integer | `100000000` | Maximum guest instructions per callback. |
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
nicaiemu path/to/game.CBE

# Load with custom window size
nicaiemu path/to/game.CBE --width 600 --height 1000

# Run fullscreen with the xbrz pixel-art scaler
nicaiemu path/to/game.CBE --fullscreen --filter xbrz

# List resources in the archive
nicaiemu path/to/game.CBE --list
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
which touch-driven games (such as 魔塔, 孤岛, 大家来数钱, and 魔鬼理发师) use
for menus and in-game controls. 孤岛's main menu uses one click to select an
item and a second click to activate it.

The `--remap` option rebinds a guest key to any host key using
`GUEST_KEY:KEY` syntax. Guest key names are `0`–`9`, `q`, `e`, `enter`,
`left`, `right`, `up`, `down`, `n`, and `m`; host keys use names such as
`space`, `x`, `f1`, or `backspace`. Escape is always reserved for exiting:

```bash
# Confirm with Space instead of Enter/F
nicaiemu path/to/game.CBE --remap enter:space

# Put the dpad on WASD and confirm on Space
nicaiemu path/to/game.CBE --remap up:w --remap down:s --remap left:a --remap right:d --remap enter:space
```

## Headless Mode

Run the emulator without a window — useful for automated testing and batch
processing:

```bash
# Run 120 frames silently and exit
nicaiemu path/to/game.CBE --headless --frames 120

# Older-style screenshot-only headless run
nicaiemu path/to/game.CBE --screenshot /dev/null --screenshot-frames 120
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

## Screen Rotation

Games packaged for the original phone's rotated landscape LCD are presented
at 400×240 automatically through a built-in content profile keyed by archive
CRC-32 and size. Use `--rotate none|cw|ccw` to override the detection for a
single run.

For a landscape game outside the built-in profile, supply extra entries with
`--rotation-profile <FILE>` instead of waiting for a core update. The file is
CSV text with one `crc32,length,rotation` entry per line (`crc32` in hex with
an optional `0x` prefix, `length` in decimal bytes, `rotation` one of `none`,
`cw`, `ccw`; blank lines and `#` comments are ignored). Compute the identity
of a game file with any CRC-32 tool:

```text
# crc32,length,rotation
282fe73d,1143317,ccw
0x9c5e0674,958874,ccw
```

User entries win over built-in ones with the same identity, so they can also
force `none` to un-rotate a misprofiled game.

## Audio

`--volume` sets the playback volume from 0 to 100. The emulator plays guest
WAV/MP3/MIDI audio at 44.1 kHz stereo through the default system output device.
If no device is available the emulator continues silently and logs a warning.

`--auto-bgm` plays the first packaged MIDI resource as background music when a
game never calls the audio manager on its own. Some games (for example the
local 魔塔 build) ship `.mid` soundtrack resources but never issue audio-manager
calls, so the music stays silent without this option. The layer restarts the
MIDI after each pass and hands audio back to the game as soon as the guest
issues its own audio call.

## Input Timing

The guest screen scheduler runs every 100ms. A pressed key produces one
`KeyDown` edge, while `KeyHold` remains active on every guest tick until the
physical key is released. This preserves continuous movement and walking
animation without synthesizing host-side repeat pulses.

## Screenshot Mode

Capture a PNG screenshot after a number of frames, then exit:

```bash
# Take a screenshot after 120 frames
nicaiemu path/to/game.CBE --screenshot screenshot.png --screenshot-frames 120
```

This is used by the batch screenshot script (`scripts/batch-screenshots.ps1`)
to generate screenshots for all games at once. Screenshots always use the
native framebuffer resolution.

## Headless Validation Tool

The `cbe_boot` tool runs the same machine core without opening a window. A key
event uses `FRAME:PHONE_KEY` syntax. A held range uses
`START_FRAME:END_FRAME:PHONE_KEY`, with the end frame excluded.

```bash
cargo run --release -p nicaiemu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 120 --key-event 1:14 --screenshot frame.png

cargo run --release -p nicaiemu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 120 --key-hold 30:60:16 --screenshot held.png
```

Set `CBE_TRACE=all` to trace every bridged service, or provide comma-separated
service filters such as `CBE_TRACE=4:24,6:3`. Tracing is disabled by default.

## Batch Screenshots

To capture every CBE application in the local validation directory, run:

```powershell
pwsh scripts/batch-screenshots.ps1
```

The script builds the standalone `nicaiemu` executable, runs each application
with default or application-specific capture timing, and writes PNG captures
to `docs/images`. Headless capture keeps the last guest-rendered frame if a
later callback stops. Blank frames and applications that stop before drawing
are reported as failures. Use `-Frames`, `-Binary`, `-GameDirectory`, or
`-OutputDirectory` to override the script defaults where applicable.

## Examples

```bash
# Basic usage
nicaiemu path/to/game.CBE

# Custom window size
nicaiemu path/to/game.CBE --width 600 --height 1000

# Fullscreen with the xbrz filter and gamepad overlay
nicaiemu path/to/game.CBE --fullscreen --filter xbrz --show-gamepad

# Take a screenshot and exit
nicaiemu path/to/game.CBE --screenshot shot.png --screenshot-frames 120

# Run and write a save state when the window closes
nicaiemu path/to/game.CBE --save-state game.sav

# Resume from a save state
nicaiemu path/to/game.CBE --load-state game.sav

# List archive contents
nicaiemu path/to/game.CBE --list

# Run 120 frames without a window
nicaiemu path/to/game.CBE --headless --frames 120

# Verbose logging
nicaiemu path/to/game.CBE --verbose
```
