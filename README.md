# NicaiEmu

A desktop emulator for Nicai/MStar CBE format mobile games, written in Rust.

## Overview

NicaiEmu is an emulator for games built with the Cool Bar Engine (CBE) format, used on Nicai/MStar mobile phones. The project implements CBE archive loading, resource parsing, and aims to provide a complete XSE virtual machine for script execution.

## Features

- **CBE Archive Loading**: Parse and load CBE game containers
- **Resource Parsing**: Handle .sce (scenes), .map (maps), .actor (sprites), .xse (scripts)
- **Scene Rendering**: Display game scenes with proper resolution (240x400 WQVGA)
- **XSE Virtual Machine**: Execute game scripts (planned)

## Architecture

The project follows a modular architecture with platform-independent core and frontend crates:

```
nicaiemu-core/      # Platform-independent emulator core
├── cbe/           # CBE format parsing
├── vm/            # XSE virtual machine (planned)
└── runtime.rs     # Runtime state management

nicaiemu/           # Desktop application
└── main.rs        # Window management and main loop

nicaiemu-libretro/  # Libretro integration (planned)
```

## Building

### Prerequisites

- Rust 1.70+ (with cargo)
- For desktop: X11/ALSA development libraries (Linux)

### Build

```bash
# Build desktop application
cargo build --release -p nicaiemu

# Build libretro core
cargo build --release -p nicaiemu-libretro
```

### Run

```bash
# Run with a CBE file
./target/release/nicaiemu --file path/to/game.CBE

# Run with specific scene
./target/release/nicaiemu --file game.CBE --scene guangmingshendian.sce

# Enable verbose logging
./target/release/nicaiemu --file game.CBE -v
```

## Project Status

**Phase 1: CBE Loader** (In Progress)
- [x] Project structure and architecture
- [x] CBE archive loading
- [x] Resource type identification
- [ ] Full CBE section parsing
- [ ] GIF palette fixup
- [ ] Scene/MAP/Actor parsing

**Phase 2: XSE Virtual Machine** (Planned)
- [ ] XSE script parsing
- [ ] Group dispatcher
- [ ] Operand resolution
- [ ] Writeback mechanism

**Phase 3: Complete Emulator** (Planned)
- [ ] Input handling
- [ ] Audio playback
- [ ] Save/load state
- [ ] Libretro integration

## Technical Details

### CBE File Format

CBE files are container archives containing game resources:

- **Signature**: `FE FE FE FE FE FE FE FE` marks section boundaries
- **Sections**: Each section contains a header, offset table, name table, and resource data
- **Resources**: Games contain scenes (.sce), maps (.map), actors (.actor), scripts (.xse), images (.gif), and audio

### Screen Resolution

Nicai phones use WQVGA resolution: **240x400 pixels**

### XSE Virtual Machine

The XSE VM executes game scripts with:
- Group dispatcher for command routing
- Operand stack for parameter passing
- Writeback mechanism for state updates

## License

BSD-3-Clause
