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
| APU | Implemented | Two pulses, triangle, noise, DMC sample playback, frame sequencer and frame IRQ, non-linear mixer at 44.1 kHz |
| Controllers | Implemented | Two standard controllers with correct strobe/shift behavior |
| Mappers | Partial | 0 NROM, 1 MMC1, 2 UxROM, 3 CNROM, 4 MMC3 (incl. scanline IRQ), 5 MMC5 (partial), 7 AxROM, 24/26 VRC6 (partial) |
| Audio output | Implemented | cpal on Windows/macOS; runtime-loaded ALSA on Linux (no dev headers required) |
| Save states | Implemented | Versioned JSON format, ROM fingerprint check, 10 slots per ROM, atomic writes |
| Settings and library | Implemented | Persisted in the per-user configuration directory |
| GUI | Implemented | Dark desktop UI with library, game view, settings, save states, achievements, about |
| Gamepad input | Not implemented | Keyboard only in this build |
| PAL timing | Not implemented | NTSC timing (60.0988 Hz) only |
| Rewind/netplay/NSF/expansion audio | Not implemented | On the roadmap |

### Accuracy boundaries

The PPU advances dot by dot for timing, but composes each visible scanline in
one step. Register writes between scanlines work (status-bar splits, scroll
changes), while per-dot mid-scanline tricks are not emulated. Sprite overflow
and sprite-0 hit detection are approximated. MMC3's IRQ is clocked once per
rendered scanline rather than from an exact A12 filter. These limits are why NES
Core does not claim full NES compatibility.

MMC5 support is deliberately partial. PRG banking, 1 KB CHR banking with
separate sprite and background banks, banked PRG RAM, the 8x8 multiplier, the
scanline IRQ and `$5105` nametable mapping work. MMC5 expansion audio, vertical
split-screen rendering, ExRAM extended attributes and fill-mode nametables are
not implemented, CHR banking is always evaluated at 1 KB granularity, and the
PRG RAM write-protect registers are stored but not enforced. VRC6 supports
PRG/CHR banking, mirroring and scanline IRQs; its expansion audio and the
cycle-counting IRQ mode are not implemented, and the alternate `$B003` CHR
banking modes are treated as 1 KB banks.

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
| D-pad | Arrow keys |
| A | X |
| B | Z |
| Start | S |
| Select | A |

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
(0/1/2/3/4/5/7/24/26, with MMC5 and VRC6 partial) and the PPU's
scanline-granular rendering are the main practical limits. Bug
reports that include a legally redistributable test ROM are the most useful
kind.

## Roadmap

- More mappers (9, 10, 11, 66, 69, 71)
- MMC5 split-screen and ExRAM attributes; MMC5/VRC6 expansion audio
- Per-dot PPU rendering for exact raster effects
- Gamepad support (SDL/`gilrs`) and configurable controller ports
- PAL/Dendy timing modes
- Rewind buffer and save-state thumbnails
- Automated compatibility runs against freely licensed test ROMs

## Contributing

Issues and pull requests are welcome. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first. Do not attach copyrighted ROMs, BIOS
files or ripped artwork.

## License

NES Core is released under the [MIT License](LICENSE). Third-party Rust crates
keep their own licenses; see `Cargo.lock` for the dependency set.
