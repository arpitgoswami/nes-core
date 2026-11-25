# Changelog

All notable changes to NES Core are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/).

## [0.1.0] - 2026-11-25

### Added

- iNES and NES 2.0 ROM parsing with header and size validation.
- Ricoh 2A03 CPU with all official instructions, common unofficial
  instructions, cycle accounting, NMI/IRQ/reset and DMA stalls.
- CPU memory bus with RAM mirroring, PPU/APU/controller routing and OAM DMA.
- Scanline-rendering PPU: background, sprites (8x8/8x16), scrolling,
  nametables, attributes, palettes, sprite 0 hit, vblank/NMI, emphasis.
- APU with two pulse channels, triangle, noise, DMC, frame sequencer,
  frame IRQ and non-linear mixer.
- Standard controllers with strobe/shift behaviour.
- Mappers: NROM, MMC1, UxROM, CNROM, MMC3 (with IRQ), AxROM.
- Desktop application: ROM library with artwork fallback, import and
  drag-and-drop, game view with pixel aspect and integer scaling, pause,
  reset, fullscreen and performance readouts.
- Versioned save states with ROM fingerprint validation and ten slots.
- Persisted settings, rebindable keyboard controls, volume control, VSync and
  scaling options.
- Six generic achievements that reflect events the emulator actually tracks.
- Platform audio output: cpal on Windows/macOS; runtime-loaded ALSA on Linux.
- Unit and integration tests for the CPU, cartridges, PPU output, NMI, audio
  sample generation, controllers and save states.
- GitHub Actions CI for formatting, clippy, tests and a Windows release build.

### Known limitations

- No gamepad, PAL timing, rewind, netplay, NSF playback or expansion audio.
- PPU rendering is scanline-granular, not dot-exact within a scanline.
- Commercial game compatibility has not been broadly validated.
