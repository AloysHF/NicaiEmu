# Game File Format — CBE (Cool Bar Engine)

Nicai/MStar games use the **CBE** container format. This document describes the
file structure, section layout, and resource types.

## Overview

Each game is a single `.CBE` file containing:

1. Section markers (`FE FE FE FE FE FE FE FE`) separating resource groups
2. Resource sections with headers, offset tables, name tables, and data
3. Executable code (ARM/Thumb) and packaged resources (scenes, maps, actors, scripts, images, audio)

## File Structure

```
┌─────────────────────────────────────┐
│ Section 0: System Resources         │
│  ├─ Header                          │
│  ├─ Offset Table                    │
│  ├─ Name Table                      │
│  └─ Resource Data                   │
├─────────────────────────────────────┤
│ Section Marker: FE FE FE FE FE FE FE FE │
├─────────────────────────────────────┤
│ Section 1: Game Resources           │
│  ├─ Header                          │
│  ├─ Offset Table                    │
│  ├─ Name Table                      │
│  └─ Resource Data                   │
├─────────────────────────────────────┤
│ ...                                 │
└─────────────────────────────────────┘
```

## Section Header

Each section contains a header with metadata:

```
Offset   Size   Field
─────────────────────────────────────────────────
0x00     4      Resource count
0x04     4      Section flags
0x08     4      Name table offset
0x0C     4      Data offset
0x10     4      Data size
...
```

## Resource Types

| Extension | Type | Description |
|-----------|------|-------------|
| `.sce` | Scene | Game screen definition with actors and scripts |
| `.map` | Map | Tile-based background data |
| `.actor` | Actor | Interactive game object with behavior |
| `.xse` | Script | XSE (eXecutable Script Engine) bytecode |
| `.gif` | Image | CBE-encoded GIF image (RGB565) |
| `.wav` | Audio | WAV audio file |
| `.mid` | Audio | MIDI music file |

## Executable Format

CBE executables contain ARM/Thumb code designed for Nicai/MStar phones:

- **Display**: 240×400 (WQVGA)
- **Pixel format**: RGB565 (16-bit, 5-6-5 bit layout)
- **CPU**: ARM with Thumb instruction set
- **Byte order**: Little-endian or big-endian (detected from header)

### Memory Map

```
Address Range              Size    Description
──────────────────────────────────────────────────
0x00000000 – 0x00FFFFFF   16 MB   RAM (code + data)
0x01000000 – 0x01FFFFFF   16 MB   Video RAM (VRAM)
```

## XSE Virtual Machine

XSE scripts run on a bytecode VM with:

- **Group Dispatcher**: Routes commands to handlers
- **Operand Stack**: Manages script parameters
- **Writeback Mechanism**: Updates game state
- **Provider Services**: Host-side services for resource loading

## Graphics Format

- **Pixel format**: RGB565 (16-bit, 5-6-5 bit layout)
- **Resolution**: 240×400 (WQVGA)
- **Framebuffer**: Located in guest RAM, converted to RGB888 for display

The renderer reads RGB565 pixels from the framebuffer address in emulated memory and
converts them to RGB888 (24-bit) for output:

```
RGB565 pixel → R = (pixel >> 11) & 0x1F  → scale to 8-bit
                G = (pixel >> 5)  & 0x3F  → scale to 8-bit
                B = pixel & 0x1F          → scale to 8-bit
```

## System Services

The emulator implements firmware-style services for:

- Memory allocation and management
- Resource lookup by identifier and name
- Stream reads and compressed data
- RGB565 screen and image drawing
- GBK text measurement and rendering
- Phone-key input queries
- Screen transitions and resource notifications
- Formatted string output
