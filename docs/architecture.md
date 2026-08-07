# Architecture

NicaiEmu executes native CBE applications instead of replacing their game logic with a scene preview. The core is platform-independent and exposes a framebuffer plus phone-key input to frontends.

## Boot flow

1. `CbeArchive` scans flat, grouped, and nested resource-package sections and records resource names and ranges.
2. `CbeExecutable` validates the executable header, segment bounds, checksums, and guest byte order.
3. `NicaiMachine` maps code, initialized data, stack, heap, manager tables, and service trampolines into the guest address space.
4. The ARM/Thumb interpreter runs the application initializer and entry point, including interworking branches and compiler-generated PC-relative jump tables.
5. Each frontend frame invokes the active screen's logic and render callbacks.

## Guest memory

The machine uses checked sparse regions rather than reserving the entire 32-bit address space. Guest reads and writes honor the executable's byte order. Initialized data, stack, heap, service-manager state, and framebuffer storage are writable. Unmapped accesses are recorded for diagnostics, and an unmapped instruction fetch stops execution with an error.

## Service bridge

Native applications receive guest-callable tables whose entries lead to emulator trampolines. Implemented service families include:

- heap and memory-block allocation;
- resource lookup by identifier and name;
- length-prefixed stream reads and compressed actor streams;
- RGB565 screen and image drawing;
- GBK text measurement and rendering;
- phone-key edge and held-state queries;
- screen transitions and resource notifications;
- a bounded subset of C-style formatted strings.

Unsupported service entries currently return a neutral value. Service usage counters and opt-in tracing make missing behavior observable during compatibility work.

## Rendering

The guest owns a 240×400 RGB565 screen. Image resources are reconstructed from the CBE GIF variant or firmware PNG representation, decoded, and copied into guest image objects. The desktop frontend converts the completed screen to 32-bit RGB for `minifb`. Text is decoded as GBK and rasterized from an embedded Unicode font, so the core does not depend on host fonts.

## Headless execution

`cbe_boot` loads the same archive and machine used by the desktop frontend. It can schedule phone-key presses at exact frames, run a fixed number of callbacks, print machine diagnostics, and save a screenshot. This is the preferred path for deterministic runtime regression checks.

The desktop screenshot mode preserves the last framebuffer produced before a guest callback error. It only writes a PNG when guest execution has produced a framebuffer with more than one color; blank frames and startup failures are reported without creating a screenshot.
