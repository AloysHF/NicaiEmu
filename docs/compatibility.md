# Compatibility

## Supported application profile

The current core targets little-endian ARM/Thumb CBE executables designed for a 240×400 display. It supports applications that use the implemented native service subset for packaged images, maps, actors, text, screen changes, and keypad input.

Validation currently covers the following end-to-end behavior:

- executable initialization and entry-point return;
- title and narrative screens;
- transition into the main game screen;
- compressed actor and image loading;
- Chinese text and HUD rendering;
- directional input and continued frame execution.

## Known limitations

- Audio and MIDI playback are not implemented.
- Save states and persistent storage are not implemented.
- Libretro exports are still a scaffold and are not a usable frontend.
- Some firmware service families return neutral fallback values.
- Big-endian executable checksums can be recognized, but big-endian guest execution has not been validated.
- SCE/MAP/XSE resource parsers are inspection helpers; native executables run through the CPU core and service bridge.
- Compatibility with other resolutions and engine revisions is not guaranteed.

## Reporting a compatibility issue

Include the application resolution, the last visible screen, the input that triggers the problem, and the error text. When possible, reproduce it with `cbe_boot` and a short sequence of `--key-event FRAME:KEY` options. Do not attach copyrighted game packages to public issue reports.
