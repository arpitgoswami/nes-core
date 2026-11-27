# NES Core

**An independent NES/Famicom emulator written in Rust, with a native desktop UI.**

NES Core is not a launcher for another emulator. The CPU, PPU, APU, memory bus,
controllers, cartridge parsing and mappers are implemented in this repository.
It loads iNES/NES 2.0 `.nes` files that you already own, renders them, accepts
keyboard input and produces audio.

[Live progress dashboard](https://arpitgoswami.github.io/nes-core/) ·
[Releases](https://github.com/arpitgoswami/nes-core/releases) ·
[Changelog](CHANGELOG.md) ·
[Architecture](docs/architecture.md) ·
[Build notes](docs/building.md)

> **Disclaimer:** NES Core is an independent open-source project. It is not
> affiliated with, endorsed by, or licensed by Nintendo. "Nintendo
> Entertainment System" and "Famicom" are trademarks of Nintendo. No ROMs,
> BIOS files or commercial artwork are distributed with this project.

## Contents

- [At a glance](#at-a-glance)
- [Project status](#project-status)
- [Features](#features)
- [Accuracy boundaries](#accuracy-boundaries)
- [Installation](#installation)
- [Building from source](#building-from-source)
- [Usage](#usage)
- [Controls](#controls)
- [Testing and CI](#testing-and-ci)
- [Compatibility](#compatibility)
- [Roadmap](#roadmap)
- [Project layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

## At a glance

| | |
| --- | --- |
| Language | Rust 2021, minimum 1.80 |
| Interface | egui desktop app with a wgpu-backed renderer |
| Version | `0.5.0` in `Cargo.toml` (source on `main`, not yet tagged) |
| Tests | 70 passing: 33 cartridge, 16 mapper, 12 CPU, 5 emulator, 3 PPU, 1 controls |
| Mappers | 42 iNES mapper ids: 15 hand-written boards plus 27 through a shared extra-board engine |
| Timing | NTSC and PAL, header detected |
| Audio | 44.1 kHz non-linear mixer, plus MMC5 and VRC6 expansion audio |
| License | MIT |

## Project status

The same 0 to 10 scale the [dashboard](https://arpitgoswami.github.io/nes-core/)
shows, scored from working code and passing tests rather than from a plan. Six
steps are closed, step seven is open, which puts the project at roughly **6.5 of
10**.

| Level | Step | State |
| --- | --- | --- |
| 0 | Baseline: empty repository | Closed |
| 1 | Cartridge loading: iNES and NES 2.0 parsing, validation, mapper id | Closed |
| 2 | CPU: 6502 opcodes, cycle penalties, interrupts, DMA stalls | Closed |
| 3 | Rendering: background, sprites, scrolling, palettes, vblank NMI | Closed |
| 4 | Sound: APU channels, frame sequencer, mixer, audio output | Closed |
| 5 | Desktop app: library, game view, settings, save states | Closed |
| 6 | Mapper breadth: 42 ids covering the common and the awkward boards | Closed |
| 7 | Timing accuracy: per-dot PPU output, exact MMC3 A12 IRQ | In progress |
| 8 | Input and packaging: gamepad, per-platform builds, a tagged release | Not started |
| 9 | Compatibility: a broad, legally redistributable test library | Not started |
| 10 | v1.0: per-dot PPU, rewind, netplay, NSF, published compatibility table | Not started |

What that means in practice: emulation works end to end, and the remaining work
is accuracy and release polish. There is no v0.5.0 tag or binary yet; the newest
packaged build is the v0.3.0 Windows zip.

## Features

Status is deliberately conservative. "Implemented" means the code exists and is
covered by the tests in `tests/`; it does not mean every commercial game works.

| Area | Status | Notes |
| --- | --- | --- |
| iNES parser | Implemented | Validates signature, sizes and mapper; reads NES 2.0 size nibbles; skips trainers; rejects unlisted mapper ids with a clear error |
| CPU (2A03) | Implemented | All official 6502 instructions, common unofficial instructions, addressing modes, page-cross and branch cycle penalties, NMI/IRQ/reset, OAM/DMC DMA stalls |
| Memory bus | Implemented | 2 KB RAM mirroring, PPU/APU/controller registers, cartridge mapping |
| PPU | Implemented | Background, sprites (8x8 and 8x16), scrolling, nametables, attributes, palettes, sprite 0 hit, vblank/NMI, emphasis |
| APU | Implemented | Two pulses, triangle, noise, DMC sample playback, frame sequencer and frame IRQ, non-linear mixer at 44.1 kHz, plus MMC5 and VRC6 expansion audio |
| Controllers | Implemented | Two standard controllers with correct strobe/shift behavior |
| Mappers | Partial | 42 ids. Hand-written: 0 NROM, 1 MMC1, 2 UxROM, 3 CNROM, 4 MMC3, 5 MMC5, 7 AxROM, 9 MMC2, 10 MMC4, 11 Color Dreams, 24/26 VRC6, 66 GxROM, 71 Camerica, 232 Quattro. Shared extra-board engine: 16/159 Bandai FCG, 21/22/23/25 VRC2/VRC4, 33 Taito TC0190, 34 BNROM/NINA-001, 69 FME-7, 70/152 Bandai, 75 VRC1, 78 Irem, 79/113/146 NINA-03/06, 85 VRC7, 87/140 Jaleco, 88/154/206 Namco 118, 89/93 Sunsoft-2, 94 UN1ROM, 180 UNROM, 184 Sunsoft-1 |
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
bank, and MMC4 additionally gates its PRG RAM through `$A001`. Sprite-side latch
triggers do not fire because this PPU only fetches sprite rows 0-7, which never
reach the `$0FD8`/`$0FE8` addresses the latches watch. Camerica (71) switches
the `$C000` 16 KB bank, keeps the last bank at `$8000` and honours Fire Hawk's
one-screen mirroring register at `$9000`. Color Dreams (11) and GxROM (66)
switch a 32 KB PRG bank and an 8 KB CHR bank. Quattro (232) switches a 64 KB
outer bank at `$8000-$BFFF` and a 16 KB inner bank at `$C000-$FFFF`, but ignores
the Aladdin submapper's swapped outer-bank bits.

The extra-board engine covers 27 further ids: PRG and CHR banking, mirroring
and, where the board needs it, IRQ generation. It does not model VRC7 or FME-7
expansion audio, so those boards run silently. Mapper support means the mapping
logic exists; it does not mean every game on that board has been played.

Expansion audio runs at CPU-cycle resolution alongside the APU. The MMC5 mix
uses the same non-linear pulse DAC as the 2A03 and adds its PCM DAC as an
AC-coupled signal; VRC6's linear 6-bit DAC is scaled to a comparable level.
Both are approximations of the analogue output stage, not measured models.

## Screenshots

No screenshots are committed yet. The interface was compiled and its emulation
core tested in a headless environment, so no image would be honest. When
capturing them, place PNGs in `screenshots/` and embed them here; do not use
screenshots of commercial games without permission.

## Installation

The GitHub Actions workflow builds a Windows release binary on every change and
uploads it as the `nes-core-windows` workflow artifact. Tagged releases publish
that build as `nes-core-windows.zip` on the
[releases page](https://github.com/arpitgoswami/nes-core/releases); the archive
contains `nes-core.exe`. The newest packaged build is v0.3.0. Linux and macOS
builds are done from source.

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

## Testing and CI

The suite is 70 tests in six integration files:

| File | Tests | Covers |
| --- | --- | --- |
| `tests/cartridge.rs` | 33 | iNES and NES 2.0 headers, mirroring, PRG/CHR mapping, save-state round trips per board |
| `tests/mappers.rs` | 16 | Banking, IRQs and latching for the hand-written boards and the extra-board engine |
| `tests/cpu.rs` | 12 | Instruction results, cycles, flags, page-cross and branch penalties, interrupts |
| `tests/emulator.rs` | 5 | Whole-machine frames, NMI timing, DMA stalls, PPU output, save states |
| `tests/ppu.rs` | 3 | Background output, scroll register behaviour, sprite handling |
| `tests/controls.rs` | 1 | Controller strobe and shift order |

The `CI` workflow in [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs
on every push to `main`, every pull request and on demand. One job runs
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test --all-targets` on Linux. A second job builds the release binary on
Windows, reruns the test suite there, and uploads the build as an artifact.

What the tests do **not** cover: the GUI never runs in CI (the window is never
opened in a headless job), audio output is never listened to, and no commercial
ROM is used anywhere. Those three gaps are stated in the status table on the
dashboard as well.

## Compatibility

NES Core has been verified with the synthetic NROM test images built inside its
integration tests. Those exercise CPU instructions and cycles, interrupt entry,
DMA stalls, cartridge mapping, PPU background output, NMI handling, the
controller shift register and save states.

It has **not** been validated against a broad library of commercial games, and
no game-specific compatibility claims are made. Mapper coverage (42 mapper ids,
see the table above) and the PPU's scanline-granular rendering are the main
practical limits. Both NTSC and PAL timing are supported. Bug reports that
include a legally redistributable test ROM are the most useful kind.

## Roadmap

Planned work, in no strict order:

- Per-dot PPU rendering for exact raster effects, and an A12-filtered MMC3 IRQ
- Gamepad support (SDL or `gilrs`) and configurable controller ports
- A tagged release with Windows, macOS and Linux binaries, so the newest download
  is not the v0.3.0 zip
- More mappers: 18, 19, 64, 65, 67, 68, 118, 119, 210 and multicarts, plus VRC7
  and FME-7 expansion audio
- Finer MMC5 split-screen timing (CL-board fine scroll) and enforced PRG RAM
  write protection
- Dendy timing and PAL colour differences
- Rewind buffer and save-state thumbnails
- Automated compatibility runs against freely licensed test ROMs

## Project layout

```
src/
  main.rs           Desktop entry point and native window options
  app/              egui application: library, game screen, settings, save states
  emulator/         Joins CPU + bus and runs frames / pulls audio
  cpu/              6502 core and the opcode table
  bus/              CPU memory map, PPU/APU register routing, DMA
  ppu/              Picture processing unit and palette
  apu/              Audio channels, frame sequencer, mixer
  cartridge/        iNES parsing, mirroring, mappers, extra-board engine
  controller/       Standard controller shift register
  audio/            Platform audio output (cpal / runtime ALSA)
  save_state/       Versioned save-state format
  settings/         Persisted settings, ROM library, achievement records
tests/              CPU, cartridge, mapper, PPU, controller and machine tests
docs/               Architecture and build documentation
```

At runtime the CPU executes an instruction, the bus forwards the resulting
cycles to the PPU and APU, and `Emulator::run_frame` stops when the PPU enters
vblank. See [docs/architecture.md](docs/architecture.md) for the timing model
and data flow.

## Contributing

Issues and pull requests are welcome. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first. Do not attach copyrighted ROMs, BIOS
files or ripped artwork.

Useful bug reports include the mapper number, the mapper id from the ROM header,
a description of what the screen or sound does wrong, and a freely licensed test
ROM when one reproduces the problem.

## License

NES Core is released under the [MIT License](LICENSE). Third-party Rust crates
keep their own licenses; see `Cargo.lock` for the dependency set.
