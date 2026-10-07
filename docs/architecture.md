# Architecture

Resource-package lookup recognizes the exact extended file-package marker
(`1` at offset 84). Compact in-memory descriptors can place unrelated pointer
storage at that offset; treating every nonzero byte as a file marker incorrectly
routes resident resources through file I/O and prevents dynamic code loading.

The legacy GameManagerOld text-box constructor (index 71) uses the same bounded
initializer as the direct text-box method path. Its methods end at offset
0x34; applying the generic 0x100-byte constructor filler corrupts neighboring
screen state even when the constructor itself returns without an error.

Legacy picture-library methods maintain a bounded image-index table and resource
ID cache, decode images through the shared stream decoder, and draw through the
RGB565 blitter. The constructor stores its scanline at offset 0, target at 4,
capacity at 8, ID/image arrays at 12/16, count at 20, and release marker at 22.
Release frees the owned allocations and clears these fields; unknown image
indices and exhausted capacities do not allocate fallback objects.

NicaiEmu executes native CBE applications instead of replacing their game logic with a scene preview. The core is platform-independent and exposes a framebuffer plus phone-key input to frontends.

## Boot flow

1. `CbeArchive` scans flat, grouped, and nested resource-package sections and records resource names and ranges.
2. `CbeExecutable` validates the executable header, segment bounds, checksums, and guest byte order.
3. `NicaiMachine` maps code, initialized data, stack, heap, manager tables, and service trampolines into the guest address space.
4. The ARM/Thumb interpreter runs the application initializer and entry point, including interworking branches and compiler-generated PC-relative jump tables.
5. Applications that contain an installer can extract their named resource packages into the sandboxed guest filesystem before launching the installed entry point.
6. Each 100ms guest tick invokes the active screen's logic and render callbacks.


Pending resource callbacks bound to a new screen run before its initialization,
so initialization can use the objects created by resource loading. Requests
issued during initialization still run before logic and rendering.

## Guest memory

The machine uses checked sparse regions rather than reserving the entire 32-bit address space. Guest reads and writes honor the executable's byte order. Initialized data, stack, heap, service-manager state, and framebuffer storage are writable. Unmapped accesses are recorded for diagnostics, and an unmapped instruction fetch stops execution with an error.

## Guest termination

The ARM C runtimes linked into CBE applications terminate through the Angel semihosting interface: ARM `SWI #0x123456` or Thumb `SVC #0xAB` with operation `0x18` (ReportException). The machine treats such an exit as a normal halt: the frame loop stays idle and error-free, and frontends keep presenting the last framebuffer. Semihosting `WRITEC`/`WRITE0` console output is captured under `CBE_TRACE` and is useful for reading guest-side diagnostics such as C runtime arithmetic-exception messages.

## Service bridge

Native applications receive guest-callable tables whose entries lead to emulator trampolines. Implemented service families include:

- heap and memory-block allocation;
- guest file access and directory queries backed by an in-memory sandbox;
- resource lookup by identifier and name, including file-backed data packages;
- byte- and word-length-prefixed stream reads across both DreamFactory manager table layouts;
- RGB565 screen and image drawing;
- GBK text measurement and rendering;
- phone-key edge and held-state queries, with held keys remaining visible on
  every guest tick until the frontend reports their release;
- screen transitions and resource notifications;
- a bounded subset of C-style formatted strings.

Unsupported service entries currently return a neutral value. Service usage counters and opt-in tracing make missing behavior observable during compatibility work.

The shared I/O table and all I/O initializer entry points use the same method
addresses. The first 24 methods have a 0x100-byte spacing because guest startup
code identifies the NV implementation by subtracting adjacent function pointers
and derives its read/write callbacks from method 17. Private tables retain this
layout while each initializer writes only its declared number of slots.

## Rendering

Unknown object method slots are assigned the inert object-method kind, never
the global GameManager kind. Object arguments must not trigger global
constructor heuristics that write through caller stack slots. These inert
methods do not establish support for the object's unimplemented behavior.

Fixed-address picture-library services share the legacy picture implementation
for load/cache, image sizes, drawing targets and owned-resource release. The
constructor initializes the default target and ownership marker explicitly.

Fixed legacy window repaint clips each dirty rectangle, invokes its guest
paint callback synchronously with the configured context, and visits child
and sibling windows. CPU registers are restored after each nested painter;
dirty queues are cleared after drawing. Guest callback errors propagate.

Legacy text boxes retain GBK byte offsets and byte lengths in owned line
tables, expose page counts in the guest object, and render the selected page
with horizontal/vertical alignment and RGB888 colors. Constructors receive
the final geometry arguments on the stack. Drawing respects the configured
line height; releasing a box frees its line tables without freeing guest text.

The guest owns a 240×400 RGB565 screen. Image resources are reconstructed from the CBE GIF variant or firmware PNG representation, decoded, and copied into guest image objects. Some custom GIF headers resemble ICO files, so a failed standard-image decode falls back to the CBE decoder. The desktop frontend converts the completed screen to 32-bit RGB for `minifb`. Text is decoded as GBK and rasterized from an embedded Unicode font, so the core does not depend on host fonts. Text coordinates live in the presented display space: for landscape-packaged games the glyphs are mapped from the 400×240 display back into the 240×400 framebuffer pixel by pixel, and games that submit `DrawText` with zero coordinates inherit the pen origin latched from the preceding `GetScreenImage` call.

## Headless execution

Screen logic and rendering callbacks are optional while firmware dialogs
suspend the underlying screen. Deferred callbacks continue to run and can
restore the screen; frames without rendering preserve existing pixels.

The fixed game manager routes its string and font queries to the GameLCD renderer.
Font queries return pixel metrics rather than echoing leftover stub addresses.
Legacy GameLCD uses separate register/stack arguments for tile blits and
bounded GBK strings (text, byte length, x, y, RGB888). Its clip rectangle lives
in reserved screen-descriptor bytes and is therefore preserved by memory
snapshots. Clip adjustment advances source coordinates as well as destination
coordinates; transparent blits preserve destination pixels for zero RGB565.

`cbe_boot` loads the same archive and machine used by the desktop frontend. It can schedule phone-key presses at exact frames, run a fixed number of callbacks, print machine diagnostics, and save a screenshot. This is the preferred path for deterministic runtime regression checks.

The desktop screenshot mode preserves the last framebuffer produced before a guest callback error. It only writes a PNG when guest execution has produced a framebuffer with more than one color; blank frames and startup failures are reported without creating a screenshot.
