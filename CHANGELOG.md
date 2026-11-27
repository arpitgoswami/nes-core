# Changelog

All notable changes to NES Core are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/).

## [0.4.0] - 2026-11-27

### Added

- MMC2 (mapper 9) and MMC4 (mapper 10): 8 KB and 16 KB PRG banking with the
  last banks fixed, four 4 KB CHR registers, and latch-driven CHR switching
  from PPU fetches of tiles `$FD` and `$FE`. A fetch that flips a latch still
  reads the old bank, and MMC4 gates PRG RAM through `$A001`.
- Color Dreams (mapper 11) and GxROM (mapper 66): 32 KB PRG and 8 KB CHR bank
  switching.
- Camerica (mapper 71): 16 KB switchable bank at `$C000` with the last bank at
  `$8000`, and Fire Hawk's one-screen mirroring register at `$9000`.
- `Cartridge::chr_read_ppu` so the PPU can report pattern fetches to mappers
  without exposing mutable cartridge state to the rendering loop.
- Tests for MMC2/MMC4 PRG and CHR latch behaviour, MMC4 PRG RAM gating, Color
  Dreams and GxROM banking, and Camerica banking plus mirroring.

### Known limitations

- The MMC2/MMC4 latches are not triggered by sprite pattern fetches, because
  the PPU only fetches sprite rows 0-7 and the trigger sits in row 8.
- Camerica's Quattro 64 KB outer bank (mapper 232) is not implemented.
- Mapper 71 uses the header's fixed mirroring until a Fire Hawk `$9000` write
  selects one-screen mirroring.

## [0.3.0] - 2026-11-27

### Added

- MMC5 expansion audio: two pulse channels with the MMC5's 240 Hz envelope and
  length clock, the 8-bit raw PCM channel at `$5011`, the `$5010` PCM IRQ with
  read acknowledgement, and the `$5015` enable and status register.
- MMC5 vertical split-screen: `$5200-$5202` select the side, tile threshold,
  scroll and 4 KB CHR bank, and the split region takes its tiles, attributes and
  pattern rows from ExRAM.
- MMC5 ExRAM extended attributes: `$5104` mode 1 gives every background tile its
  own palette and 4 KB CHR bank, with `$5130` supplying the upper CHR bits.
- VRC6 expansion audio: two pulse channels and the sawtooth accumulator,
  including `$9003` halt and 16x/256x frequency scaling.
- Expansion audio is clocked per CPU cycle and mixed into the APU output.
- Tests for MMC5 pulse and PCM output, the PCM IRQ, VRC6 pulse/saw output and
  halt, ExRAM attribute selection, split-region fetches, and rendered pixels for
  both MMC5 background modes.

### Known limitations

- MMC5 fill-mode nametables, ExRAM used as a nametable, PCM read mode and the
  SL board's split fine-scroll are not implemented.
- Split-screen substitution is applied per screen tile and ignores the PPU's
  dummy nametable fetches. Substitution follows PPUMASK's rendering bits rather
  than the dedicated pin.
- MMC5 expansion audio uses a fixed 7424-cycle frame clock and an AC-coupled
  model of the PCM DAC.

## [0.2.0] - 2026-11-26

### Added

- MMC5 (mapper 5): all four PRG banking modes, 1 KB CHR banking with separate
  sprite and background banks, 64 KB of banked PRG RAM, 1 KB of extended RAM at
  `$5C00-$5FFF`, per-table nametable mapping through `$5105`, the 8x8
  multiplier, frame status and scanline-compare IRQs.
- VRC6a and VRC6b (mappers 24 and 26): 16 KB and 8 KB PRG banking, 1 KB CHR
  banking, mirroring control through `$B003`, and scanline IRQs.
- Both mappers keep their registers in versioned save states.

### Known limitations

- MMC5 expansion audio, vertical split-screen, ExRAM extended attributes and
  fill-mode nametables are not implemented. CHR banking is evaluated at 1 KB
  granularity in every CHR mode, and the PRG RAM protect registers are stored
  but not enforced.
- VRC6 expansion audio and the cycle-counting IRQ mode are not implemented.
  The alternate `$B003` CHR banking modes are treated as 1 KB banks.

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
