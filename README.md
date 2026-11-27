# NES Core

An independent, open-source NES/Famicom emulator written in Rust, with a desktop
interface built on egui and a wgpu-backed renderer.

NES Core is not a launcher for another emulator: the CPU, PPU, APU, memory bus,
controllers, cartridge parsing and mappers are implemented in this repository.
It loads iNES/NES 2.0 `.nes` files that you already own, renders games, accepts
keyboard input and produces audio.

> **Disclaimer:** NES Core is an independent open-source project. It is not
> affiliated with, endorsed by, or licensed by Nintendo. "Nintendo
> Entertainment System" and "Famicom" are trademarks of Nintendo. No ROMs,
> BIOS files or commercial artwork are distributed with this project.

## Features

Status is deliberately conservative. "Implemented" means the code exists and is
covered by the tests in `tests/`; it does not mean every commercial game works.

| Area | Status | Notes |
| --- | --- | --- |
| iNES parser | Implemented | Validates signature, sizes and mapper; reads NES 2.0 size nibbles; skips trainers |
| CPU (2A03) | Implemented | All official 6502 instructions, common unofficial instructions, addressing modes, page-cross and branch cycle penalties, NMI/IRQ/reset, OAM/DMC DMA stalls |
| Memory bus | Implemented | 2 KB RAM mirroring, PPU/APU/controller registers, cartridge mapping |
| PPU | Implemented | Background, sprites (8x8 and 8x16), scrolling, nametables, attributes, palettes, sprite 0 hit, vblank/NMI, emphasis |
| APU | Implemented | Two pulses, triangle, noise, DMC sample playback, frame sequencer and frame IRQ, non-linear mixer at 44.1 kHz, plus MMC5 and VRC6 expansion audio |
| Controllers | Implemented | Two standard controllers with correct strobe/shift behavior |
| Mappers | Partial | 0 NROM, 1 MMC1, 2 UxROM, 3 CNROM, 4 MMC3, 5 MMC5, 7 AxROM, 9 MMC2, 10 MMC4, 11 Color Dreams, 16/159 Bandai FCG, 21/22/23/25 VRC2/VRC4, 24/26 VRC6, 33 Taito TC0190, 34 BNROM/NINA-001, 66 GxROM, 69 FME-7, 70/152 Bandai, 71 Camerica, 75 VRC1, 78 Irem, 79/113/146 NINA-03/06, 85 VRC7, 87 Jaleco, 88/154/206 Namco 118, 89/93 Sunsoft-2, 94 UN1ROM, 140 Jaleco, 180 UNROM, 184 Sunsoft-1, 232 Quattro (42 mappers; unlisted mappers report a clear error) |
| Audio output | Implemented | cpal on Windows/macOS; runtime-loaded ALSA on Linux (no dev headers required) |
| Save states | Implemented | Versioned JSON format, ROM fingerprint check, 10 slots per ROM, atomic writes |
| Settings and library | Implemented | Persisted in the per-user configuration directory |
| GUI | Implemented | Dark desktop UI with library, game view, settings, save states, achievements, about |
| Gamepad input | Not implemented | Keyboard only in this build |
| PAL timing | Implemented | Header-detected PAL runs 312 scanlines at 3.2 dots per CPU cycle and 50.007 Hz |
| Rewind/netplay/NSF playback | Not implemented | On the roadmap |

### Accuracy boundaries

The PPU advances dot by dot for timing, but composes each visible scanline in
one step. Register writes between scanlines work (status-bar splits, scroll
changes), while per-dot mid-scanline tricks are not emulated. Sprite overflow
and sprite-0 hit detection are approximated. MMC3's IRQ is clocked once per
rendered scanline rather than from an exact A12 filter. These limits are why NES
Core does not claim full NES compatibility.

MMC5 is implemented across the board registers. PRG banking, 1 KB CHR banking
with separate sprite and background banks, banked PRG RAM, the 8x8 multiplier,
the scanline IRQ, `$5105` nametable mapping, fill mode, ExRAM nametables, ExRAM
extended attributes, vertical split-screen, PCM read mode and expansion audio
all work. Extended attributes give every background tile its own palette and
4 KB CHR bank, and the split region takes its tiles, attributes and pattern rows
from ExRAM. Fill mode returns `$5106`/`$5107` for tiles and attributes, and
ExRAM nametables route PPU reads and writes through the cartridge. Remaining
limits: CHR banking is evaluated at 1 KB granularity, background substitution
is applied per screen tile rather than per PPU fetch, split fine scrolling uses
the SL-board path (the bottom three bits of `$5201`), the PRG RAM write-protect
registers are stored but not enforced, and the scanline IRQ is clocked once per
rendered line rather than from an exact PPU address watch. VRC6 supports PRG/CHR
banking, mirroring, scanline and cycle-counting IRQs, all four `$B003` CHR
banking modes and its two pulse channels plus sawtooth.

MMC2 and MMC4 switch CHR banks from PPU fetches of tiles `$FD` and `$FE`, one
latch per pattern-table half. The fetch that flips a latch still reads the old
bank, and MMC4 additionally gates its PRG RAM through `$A001`. Sprite-side
latch triggers do not fire because this PPU only fetches sprite rows 0-7.
Camerica (71) switches the `$C000` 16 KB bank, keeps the last bank at `$8000`
and honours Fire Hawk's one-screen mirroring register at `$9000`; the Quattro
64 KB outer bank (mapper 232) is not implemented. Color Dreams (11) and GxROM
(66) switch a 32 KB PRG bank and an 8 KB CHR bank.

Expansion audio runs at CPU-cycle resolution alongside the APU. The MMC5 mix
uses the same non-linear pulse DAC as the 2A03 and adds its PCM DAC as an
AC-coupled signal; VRC6's linear 6-bit DAC is scaled to a comparable level.
Both are approximations of the analogue output stage, not measured models.

## Screenshots

No screenshots are committed yet. The interface was compiled and its emulation
core tested in a headless environment, so no image would be honest. When
capturing them, place PNGs in `screenshots/` and embed them here; do not use
screenshots of commercial games without permission.

## Architecture

```
src/
  main.rs           Desktop entry point and native window options
  app/              egui application: library, game screen, settings, save states
  emulator/         Joins CPU + bus and runs frames / pulls audio
  cpu/              6502 core and the opcode table
  bus/              CPU memory map, PPU/APU register routing, DMA
  ppu/              Picture processing unit and palette
  apu/              Audio channels, frame sequencer, mixer
  cartridge/        iNES parsing, mirroring, mappers
  controller/       Standard controller shift register
  audio/            Platform audio output (cpal / runtime ALSA)
  save_state/       Versioned save-state format
  settings/         Persisted settings, ROM library, achievement records
tests/              CPU, cartridge and full-machine integration tests
docs/               Architecture and build documentation
```

At runtime the CPU executes an instruction, the bus forwards the resulting
cycles to the PPU and APU, and `Emulator::run_frame` stops when the PPU enters
vblank. See [docs/architecture.md](docs/architecture.md) for the timing model
and data flow.

## Installation

The GitHub Actions workflow builds a Windows release binary on every change and
uploads it as the `nes-core-windows` workflow artifact. Tagged releases publish
that build as `nes-core-windows.zip` on the
[releases page](https://github.com/arpitgoswami/nes-core/releases); the archive
contains `nes-core.exe`. Linux and macOS builds are done from source.

There is no installer and no auto-update. NES Core does not need administrator
privileges.

## Building from source

Requirements:

- Rust 1.80 or newer (stable toolchain)
- Windows: the MSVC build tools (Visual Studio "Desktop development with C++")
- Linux: a C toolchain and the usual windowing/GPU runtime libraries. Audio uses
  ALSA at runtime, so `libasound2-dev` is **not** required to build.
- macOS: Xcode command line tools

```sh
git clone https://github.com/arpitgoswami/nes-core.git
cd nes-core
cargo build --release
./target/release/nes-core
```

Checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

More detail, including CI artifacts and headless notes, is in
[docs/building.md](docs/building.md).

## Usage

1. Start NES Core; the library opens with no ROM loaded.
2. Choose **Load ROM** (or press <kbd>Ctrl</kbd>+<kbd>O</kbd>) and pick a `.nes`
   file, or drop one onto the window.
3. The game starts immediately. Invalid headers, truncated files and unsupported
   mappers produce an error message instead of a crash.

ROMs and artwork stay on your machine. A ROM's library card uses
`<rom name>.png` or `<rom name>.jpg` next to the ROM when present and otherwise
draws a generated tile.

## Controls

Default keyboard bindings (all rebindable in Settings):

| NES button | Key |
| --- | --- |
| D-pad | W A S D |
| A | K |
| B | J |
| Start | Enter |
| Select | Backspace |

Emulator shortcuts:

| Shortcut | Action |
| --- | --- |
| <kbd>Space</kbd> | Pause / resume |
| <kbd>F2</kbd> | Reset console |
| <kbd>F5</kbd> | Save slot 0 |
| <kbd>F8</kbd> | Load slot 0 |
| <kbd>F11</kbd> | Toggle fullscreen |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | Open a ROM |

Gamepad support is **not implemented** in this build. The game view reports
"P1 keyboard" for that reason.

## Compatibility

NES Core has been verified with the synthetic NROM test images built inside its
integration tests. Those exercise CPU instructions and cycles, interrupt entry,
DMA stalls, cartridge mapping, PPU background output, NMI handling, the
controller shift register and save states.

It has **not** been validated against a broad library of commercial games, and
no game-specific compatibility claims are made. Mapper coverage
(42 mappers, see the table above) and the PPU's scanline-granular
rendering are the main practical limits. Both NTSC and PAL timing are supported. Bug
reports that include a legally redistributable test ROM are the most useful
kind.

## Roadmap

- More mappers (18, 19, 64, 65, 67, 68, 118, 119, 210 and multicarts); VRC7 and FME-7 expansion audio
- MMC5 fill-mode and ExRAM nametables, PCM read mode and finer split-screen timing
- Per-dot PPU rendering for exact raster effects
- Gamepad support (SDL/`gilrs`) and configurable controller ports
- Dendy timing and PAL colour differences
- Rewind buffer and save-state thumbnails
- Automated compatibility runs against freely licensed test ROMs

## Contributing

Issues and pull requests are welcome. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first. Do not attach copyrighted ROMs, BIOS
files or ripped artwork.

## License

NES Core is released under the [MIT License](LICENSE). Third-party Rust crates
keep their own licenses; see `Cargo.lock` for the dependency set.
