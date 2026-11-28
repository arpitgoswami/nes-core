# NES Core

**An independent NES/Famicom emulator written in Rust, with a native desktop
interface.**

NES Core is a standalone emulator, not a launcher for another emulator. The
CPU, PPU, APU, memory bus, controller handling, cartridge parsing and mapper
implementations are contained in this repository. The application opens iNES
and NES 2.0 `.nes` files that the user already owns, renders them, accepts
keyboard input and produces audio.

[Live progress dashboard](https://arpitgoswami.github.io/nes-core/) ·
[Completion scale](https://arpitgoswami.github.io/nes-core/#completion) ·
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
  - [Levels](#levels)
  - [Completion bands](#completion-bands)
- [Releases and downloads](#releases-and-downloads)
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

| Property | Value |
| --- | --- |
| Language | Rust 2021, minimum version 1.80 |
| Interface | egui desktop application with a wgpu-backed renderer |
| Version | `0.5.0` in `Cargo.toml`; source on `main`, not yet tagged |
| Completion | 65 percent on the project scale, level 6.5 of 10 |
| Tests | 90 passing across 8 integration files |
| Mappers | 73 iNES mapper ids: 16 hand-written boards plus 57 through a shared extra-board engine |
| Timing | NTSC and PAL, detected from the cartridge header |
| Audio | 44.1 kHz non-linear mixer, plus MMC5 and VRC6 expansion audio |
| License | MIT |

## Project status

The completion score is derived from implemented code and passing tests rather
than from a plan. Six of the ten levels are closed, level seven is in progress,
and the resulting score is approximately 65 percent. The
[dashboard](https://arpitgoswami.github.io/nes-core/) presents the same score
as a single bar from zero to one hundred, with a marker at the current position
and the scale divided into ten bands of ten percent each.

### Levels

| Level | Scope | State |
| --- | --- | --- |
| 0 | Baseline: empty repository | Closed |
| 1 | Cartridge loading: iNES and NES 2.0 parsing, validation, mapper id | Closed |
| 2 | CPU: 6502 opcodes, cycle penalties, interrupts, DMA stalls | Closed |
| 3 | Rendering: background, sprites, scrolling, palettes, vblank NMI | Closed |
| 4 | Sound: APU channels, frame sequencer, mixer, audio output | Closed |
| 5 | Desktop application: library, game view, settings, save states | Closed |
| 6 | Mapper breadth: 73 ids covering the common and the awkward boards | Closed |
| 7 | Timing accuracy: per-dot PPU output, exact MMC3 A12 IRQ | In progress |
| 8 | Input and packaging: gamepad, per-platform builds, a tagged release | Not started |
| 9 | Compatibility: a broad, legally redistributable test library | Not started |
| 10 | v1.0: per-dot PPU, rewind, netplay, NSF, published compatibility table | Not started |

### Completion bands

| Band | Work covered | State |
| --- | --- | --- |
| 0-10% | Baseline and cartridge loading: repository, iNES and NES 2.0 headers, validation, mapper id resolution | Complete |
| 10-20% | CPU: official and common unofficial opcodes, page-cross and branch penalties, NMI, IRQ, reset, DMA stalls | Complete |
| 20-30% | Rendering: background, sprites in 8x8 and 8x16, scrolling, nametables, palettes, sprite 0 hit, vblank NMI | Complete |
| 30-40% | Sound: two pulses, triangle, noise, DMC, non-linear 44.1 kHz mixer, platform audio output | Complete |
| 40-50% | Desktop application: library, game view, settings, rebindable controls, achievements, save states | Complete |
| 50-60% | Mapper breadth: 73 mapper ids and versioned save-state support per board | Complete |
| 60-70% | Timing accuracy: frame timing and scrolling are correct; per-dot PPU output and the exact MMC3 A12 IRQ filter are open | In progress |
| 70-80% | Input and packaging: gamepad support, macOS and Linux builds, a published v0.5.0 tag and binary | Not started |
| 80-90% | Compatibility: a broad, legally redistributable ROM library and published results | Not started |
| 90-100% | v1.0: per-dot PPU, rewind, netplay, NSF playback, builds for every platform, compatibility table | Not started |

Bands covering zero to sixty percent are complete. The project currently sits
inside the 60 to 70 percent band. The remaining three bands describe the work
needed before a v1.0 claim could be made.

## Releases and downloads

Two states exist and are reported separately.

**Current source.** Version `0.5.0` is on `main`. It carries no tag and no
published binary.

**Published releases.** The tagged releases are v0.1.0, v0.2.0 and v0.3.0.
Because no tag exists beyond v0.3.0, the latest published release and the
stable release are presently the same build.

| Item | Value |
| --- | --- |
| Latest published release | v0.3.0 |
| Stable release | v0.3.0 (identical, since no newer tag exists) |
| Archive | `nes-core-windows.zip`, containing `nes-core.exe` |
| Size | 5,616,935 bytes |
| SHA-256 | `d04c787c897dd50d756fe0ec8f8a87b2a6f227ddb9e319f65b71244102154e00` |
| Platform | Windows 10 or newer, x64; no installer and no administrator rights |

- [Download the latest release](https://github.com/arpitgoswami/nes-core/releases/download/v0.3.0/nes-core-windows.zip)
- [Stable release page](https://github.com/arpitgoswami/nes-core/releases/latest)
- [All releases](https://github.com/arpitgoswami/nes-core/releases)

The releases page is authoritative for download links and checksums. Linux and
macOS builds are produced from source; see
[Building from source](#building-from-source).

## Features

Status is deliberately conservative. "Implemented" means that the code exists
and is covered by the tests in `tests/`; it does not mean that every commercial
game behaves correctly.

| Area | Status | Notes |
| --- | --- | --- |
| iNES parser | Implemented | Validates signature, sizes and mapper; reads NES 2.0 size nibbles; skips trainers; rejects unlisted mapper ids with a clear error |
| CPU (2A03) | Implemented | All official 6502 instructions, common unofficial instructions, addressing modes, page-cross and branch cycle penalties, NMI, IRQ, reset, OAM and DMC DMA stalls |
| Memory bus | Implemented | 2 KB RAM mirroring, PPU, APU and controller registers, cartridge mapping |
| PPU | Implemented | Background, sprites in 8x8 and 8x16, scrolling, nametables, attributes, palettes, sprite 0 hit, vblank, NMI, emphasis |
| APU | Implemented | Two pulses, triangle, noise, DMC sample playback, frame sequencer and frame IRQ, non-linear mixer at 44.1 kHz, plus MMC5 and VRC6 expansion audio |
| Controllers | Implemented | Two standard controllers with correct strobe and shift behaviour |
| Mappers | Partial | 73 ids. Hand-written: 0 NROM, 1 MMC1, 2 UxROM, 3 CNROM, 4 MMC3, 5 MMC5, 7 AxROM, 9 MMC2, 10 MMC4, 11 Color Dreams, 24 and 26 VRC6, 66 GxROM, 71 Camerica, 118 TxSROM, 232 Quattro. Shared extra-board engine, 57 ids: 13 CPROM, 16 and 159 Bandai FCG, 18 Jaleco SS88006, 21, 22, 23 and 25 VRC2/VRC4, 32 Irem G-101, 33 and 48 Taito TC0190, 34 BNROM/NINA-001, 38, 65 Irem H3001, 67 Sunsoft-3, 69 FME-7, 70 and 152 Bandai, 72 and 92 Jaleco, 73 VRC3, 75 and 151 VRC1, 76, 88, 95, 154 and 206 Namco 108 family, 78 Irem, 79, 113 and 146 NINA-03/06, 85 VRC7, 86, 87 and 140 Jaleco, 89 and 93 Sunsoft-2, 94 UN1ROM, 107, 112 Asder, 133, 148 and 149 Sachen, 180 UNROM, 184 Sunsoft-1, 200, 201, 202, 203, 225, 229, 240, 241 and 242 multicarts, 210 Namco 175/340, 246 |
| Audio output | Implemented | cpal on Windows and macOS; runtime-loaded ALSA on Linux, so no development headers are required |
| Save states | Implemented | Versioned JSON format, ROM fingerprint check, ten slots per ROM, atomic writes |
| Settings and library | Implemented | Persisted in the per-user configuration directory |
| GUI | Implemented | Dark desktop interface with library, game view, settings, save states, achievements and about |
| Gamepad input | Not implemented | Keyboard only in this build |
| PAL timing | Implemented | Header-detected PAL runs 312 scanlines at 3.2 dots per CPU cycle and 50.007 Hz |
| Rewind, netplay, NSF playback | Not implemented | Planned; see [Roadmap](#roadmap) |

### Accuracy boundaries

The PPU advances dot by dot for timing, but composes each visible scanline in
one step. Register writes between scanlines work, including status-bar splits
and scroll changes, while per-dot mid-scanline techniques are not emulated.
Sprite overflow and sprite 0 hit detection are approximated. The MMC3 IRQ is
clocked once per rendered scanline rather than from an exact A12 filter. These
limits are the reason NES Core does not claim full NES compatibility.

MMC5 is implemented across the board registers. PRG banking, 1 KB CHR banking
with separate sprite and background banks, banked PRG RAM, the 8x8 multiplier,
the scanline IRQ, `$5105` nametable mapping, fill mode, ExRAM nametables, ExRAM
extended attributes, vertical split-screen, PCM read mode and expansion audio
all work. Extended attributes give every background tile its own palette and
4 KB CHR bank, and the split region takes its tiles, attributes and pattern rows
from ExRAM. Fill mode returns `$5106` and `$5107` for tiles and attributes, and
ExRAM nametables route PPU reads and writes through the cartridge. Remaining
limits: CHR banking is evaluated at 1 KB granularity, background substitution
is applied per screen tile rather than per PPU fetch, split fine scrolling uses
the SL-board path, the PRG RAM write-protect registers are stored but not
enforced, and the scanline IRQ is clocked once per rendered line rather than
from an exact PPU address watch. VRC6 supports PRG and CHR banking, mirroring,
scanline and cycle-counting IRQs, all four `$B003` CHR banking modes, and its
two pulse channels plus sawtooth.

MMC2 and MMC4 switch CHR banks from PPU fetches of tiles `$FD` and `$FE`, one
latch per pattern-table half. The fetch that flips a latch still reads the old
bank, and MMC4 additionally gates its PRG RAM through `$A001`. Sprite-side latch
triggers do not fire because this PPU only fetches sprite rows 0 to 7, which
never reach the `$0FD8` and `$0FE8` addresses that the latches watch. Camerica
(71) switches the `$C000` 16 KB bank, keeps the last bank at `$8000` and honours
the Fire Hawk one-screen mirroring register at `$9000`. Color Dreams (11) and
GxROM (66) switch a 32 KB PRG bank and an 8 KB CHR bank. Quattro (232) switches
a 64 KB outer bank at `$8000-$BFFF` and a 16 KB inner bank at `$C000-$FFFF`,
but ignores the Aladdin submapper's swapped outer-bank bits.

The extra-board engine covers 57 further ids: PRG and CHR banking, mirroring
and, where the board requires it, IRQ generation. It does not model VRC7 or
FME-7 expansion audio, so those boards run silently. Mapper support means that
the mapping logic exists; it does not mean that every game using that board has
been tested.

Expansion audio runs at CPU-cycle resolution alongside the APU. The MMC5 mix
uses the same non-linear pulse DAC as the 2A03 and adds its PCM DAC as an
AC-coupled signal; the VRC6 linear 6-bit DAC is scaled to a comparable level.
Both are approximations of the analogue output stage rather than measured
models.

## Screenshots

No screenshots are committed. The interface has been compiled and its emulation
core tested in a headless environment, so presenting an image as proof of
runtime behaviour would not be accurate. When screenshots are added, place PNG
files in `screenshots/` and embed them here. Screenshots of commercial games
must not be used without permission.

## Installation

The GitHub Actions workflow builds a Windows release binary on every change and
uploads it as the `nes-core-windows` workflow artifact. Tagged releases publish
that build as `nes-core-windows.zip`, which contains `nes-core.exe`. The newest
packaged build is v0.3.0, as listed under
[Releases and downloads](#releases-and-downloads). Linux and macOS builds are
produced from source.

There is no installer and no automatic update mechanism. NES Core does not
require administrator privileges.

## Building from source

Requirements:

- Rust 1.80 or newer, stable toolchain
- Windows: the MSVC build tools, including "Desktop development with C++"
- Linux: a C toolchain and the usual windowing and GPU runtime libraries. Audio
  uses ALSA at runtime, so `libasound2-dev` is **not** required to build.
- macOS: the Xcode command line tools

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

Further detail, including CI artifacts and headless notes, is provided in
[docs/building.md](docs/building.md).

## Usage

1. Start NES Core. The library opens with no ROM loaded.
2. Choose **Load ROM** or press <kbd>Ctrl</kbd>+<kbd>O</kbd> and select a `.nes`
   file, or drop a file onto the window.
3. The game starts immediately. Invalid headers, truncated files and unsupported
   mappers produce an error message instead of a crash.

ROMs and artwork remain on the local machine. A library card uses
`<rom name>.png` or `<rom name>.jpg` beside the ROM when present, and otherwise
draws a generated tile.

## Controls

Default keyboard bindings, all rebindable in Settings:

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
| <kbd>Space</kbd> | Pause or resume |
| <kbd>F2</kbd> | Reset console |
| <kbd>F5</kbd> | Save slot 0 |
| <kbd>F8</kbd> | Load slot 0 |
| <kbd>F11</kbd> | Toggle fullscreen |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | Open a ROM |

Gamepad support is **not implemented** in this build. The game view reports
"P1 keyboard" for that reason.

## Testing and CI

The suite comprises 90 tests in eight integration files:

| File | Tests | Coverage |
| --- | --- | --- |
| `tests/cartridge.rs` | 33 | iNES and NES 2.0 headers, mirroring, PRG and CHR mapping, save-state round trips per board |
| `tests/mappers.rs` | 16 | Banking, IRQs and latching for the hand-written boards |
| `tests/mappers2.rs` | 17 | Extra-board engine: register decoding, CHR RAM paging, address-latch multicarts, and the documented mapper total |
| `tests/cpu.rs` | 12 | Instruction results, cycles, flags, page-cross and branch penalties, interrupts |
| `tests/emulator.rs` | 5 | Whole-machine frames, NMI timing, DMA stalls, PPU output, save states |
| `tests/ppu.rs` | 3 | Background output, scroll register behaviour, sprite handling |
| `tests/audio.rs` | 3 | Triangle pitch, noise rate, output range and DC offset |
| `tests/controls.rs` | 1 | Controller strobe and shift order |

The CI workflow in [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs
on every push to `main`, on every pull request and on demand. One job runs
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test --all-targets` on Linux. A second job builds the release binary on
Windows, reruns the test suite there and uploads the build as an artifact.

What the tests do **not** cover: the GUI never runs in CI, because the window is
never opened in a headless job; audio output is never listened to; and no
commercial ROM is used anywhere. The dashboard states these same three gaps.

## Compatibility

NES Core has been verified with the synthetic NROM test images built inside its
integration tests. Those images exercise CPU instructions and cycles, interrupt
entry, DMA stalls, cartridge mapping, PPU background output, NMI handling, the
controller shift register and save states.

The emulator has **not** been validated against a broad library of commercial
games, and no game-specific compatibility claims are made. Mapper coverage (73
mapper ids, listed above) and the PPU's scanline-granular rendering are the main
practical limits. Both NTSC and PAL timing are supported. Bug reports that
include a legally redistributable test ROM are the most useful kind.

## Roadmap

Planned work, in no strict order:

- Per-dot PPU rendering for exact raster effects, and an A12-filtered MMC3 IRQ
- Gamepad support through SDL or `gilrs`, and configurable controller ports
- A tagged release with Windows, macOS and Linux binaries, so that the newest
  download is no longer the v0.3.0 archive
- Further mappers, including multicarts, and VRC7 and FME-7 expansion audio
- Finer MMC5 split-screen timing on the CL board, and enforced PRG RAM write
  protection
- Dendy timing and PAL colour differences
- Rewind buffer and save-state thumbnails
- Automated compatibility runs against freely licensed test ROMs

## Project layout

```
src/
  main.rs           Desktop entry point and native window options
  app/              egui application: library, game screen, settings, save states
  emulator/         Joins CPU and bus, runs frames, pulls audio
  cpu/              6502 core and the opcode table
  bus/              CPU memory map, PPU and APU register routing, DMA
  ppu/              Picture processing unit and palette
  apu/              Audio channels, frame sequencer, mixer
  cartridge/        iNES parsing, mirroring, mappers, extra-board engine
  controller/       Standard controller shift register
  audio/            Platform audio output (cpal, runtime ALSA)
  save_state/       Versioned save-state format
  settings/         Persisted settings, ROM library, achievement records
tests/              CPU, cartridge, mapper, PPU, audio, controller and machine tests
docs/               Architecture and build documentation
```

At runtime the CPU executes an instruction, the bus forwards the resulting
cycles to the PPU and APU, and `Emulator::run_frame` stops when the PPU enters
vblank. [docs/architecture.md](docs/architecture.md) describes the timing model
and data flow.

## Contributing

Issues and pull requests are welcome. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first. Copyrighted ROMs, BIOS files and
ripped artwork must not be attached.

A useful bug report includes the mapper number, the mapper id from the ROM
header, a description of the incorrect screen or sound, and a freely licensed
test ROM when one reproduces the problem.

## License

NES Core is released under the [MIT License](LICENSE). Third-party Rust crates
retain their own licenses; `Cargo.lock` records the dependency set.
