# Architecture

NES Core is split into a hardware core (`src/lib.rs`) and a desktop front end
(`src/main.rs`, `src/app`, `src/audio`). The core has no dependency on egui and
is driven entirely through `Emulator`.

## Data flow

```
        ┌─────────────┐   reads/writes   ┌──────────────┐
        │     Cpu     │ ───────────────> │     Bus      │
        └─────────────┘                  └──────┬───────┘
               ▲                               │ cycles
               │ NMI / IRQ / DMA stalls        │
               │                        ┌──────┴───────┐
        ┌──────┴──────┐                 │ Ppu    Apu   │
        │  Emulator   │ <──── frame ────┤ Cartridge    │
        └──────┬──────┘      audio      └──────────────┘
               │
     framebuffer / samples
               │
        ┌──────┴──────┐
        │  egui + UI  │
        └─────────────┘
```

`Emulator::run_frame` executes CPU instructions, passes the cycle count returned
by each instruction to `Bus::tick`, and stops when the PPU raises its
`frame_complete` flag on entering vblank (scanline 241). The framebuffer is
copied out and the UI uploads it as a nearest-neighbour texture.

## CPU

`src/cpu` implements the 2A03's 6502 core. Opcode metadata lives in a 256-entry
table (`Op`, addressing mode, base cycles). Indexed reads add a page-cross cycle,
taken branches add one cycle plus another on a page cross, and RMW instructions
use their longer timings. Unofficial instructions that real software relies on
(LAX, SAX, DCP, ISC, SLO, RLA, SRE, RRA, ANC, ALR, ARR, SBX) are included.

Interrupts are polled at instruction boundaries. NMI is edge-triggered by the
PPU (`Bus::poll_nmi` acknowledges the edge); IRQ is level-based from the APU
frame counter, DMC and the active mapper. `BRK`, `RTI`, `RTS`, and the 6502's
indirect-`JMP` page-wrap quirk are reproduced.

DMA stalls are modelled as extra cycles: a write to `$4014` queues 513 cycles,
which the CPU adds after the current instruction. The DMC requests bytes from
the bus, which reads PRG/PRG-RAM without PPU side effects.

## Bus and cartridge

`src/bus` maps:

| Range | Target |
| --- | --- |
| `$0000-$1FFF` | 2 KB RAM, mirrored every 2 KB |
| `$2000-$3FFF` | PPU registers, mirrored every 8 bytes |
| `$4000-$4013`, `$4015`, `$4017` | APU registers |
| `$4014` | OAM DMA |
| `$4016` / `$4017` | Controller ports 1 and 2 |
| `$4020-$5FFF` | Open bus / expansion |
| `$6000-$7FFF` | Cartridge PRG RAM |
| `$8000-$FFFF` | Cartridge PRG ROM via the mapper |

`src/cartridge` parses iNES and NES 2.0 headers, validates the file length,
skips trainers, and maps PRG/CHR through `Mapper`. CHR RAM is created when the
header declares no CHR banks. Nametable mirroring is resolved by the cartridge
(`nametable_index`), which lets one-screen and four-screen cartridges work
without special cases in the PPU.

Implemented mappers: NROM (0), MMC1 (1), UxROM (2), CNROM (3), MMC3 (4), MMC5
(5), AxROM (7) and VRC6 (24/26). MMC3, MMC5 and VRC6 IRQ counters are clocked
once per rendered scanline, which approximates the hardware's A12 filter.

MMC5 owns a register window at `$5000-$5FFF`: PRG and CHR banking, 64 KB of
banked PRG RAM, the 1 KB ExRAM at `$5C00-$5FFF`, per-table nametable mapping,
the 8x8 multiplier and the scanline IRQ. Sprite pattern fetches go through
`chr_read_sprite`, so MMC5's separate sprite banks are honoured. VRC6 keeps its
registers above `$8000` and handles both address-line variants (24 and 26).
MMC5 split-screen rendering, ExRAM extended attributes and expansion audio are
not emulated.

## PPU

The PPU (`src/ppu`) steps dot by dot through 262 scanlines of 341 dots. On the
first dot of each visible scanline it composes all 256 pixels and then
increments the vertical scroll (`v`). This gives correct frame timing, vblank
and NMI placement, sprite-0 hit and per-scanline raster changes, while avoiding
the cost and complexity of a full per-dot pipeline.

Background rendering reads the nametable, attribute byte and pattern rows using
the scrolled `v` register and `fine_x`. Sprites are evaluated for the line, up to
eight are selected, 8x16 mode and horizontal/vertical flips are handled, and
background/sprite priority and palette selection follow the hardware rules.
Left-edge clipping uses PPUMASK bits 1 and 2. Greyscale and emphasis bits are
applied when converting palette indices to RGB.

Deliberate simplifications:

- Register changes *within* a scanline do not affect the pixels already composed
  for that line. Split-screen effects that change registers during hblank work.
- Sprite overflow is flagged when more than eight sprites are in range; the
  hardware's buggy overflow evaluation is not reproduced bit for bit.
- Sprite-0 hit is computed at composition time rather than at the exact dot.
- The odd-frame skipped dot is not emulated.

## APU

`src/apu` clocks pulse/triangle/noise timers at half the CPU rate, the DMC timer
at the CPU rate, and runs the 4-step/5-step frame sequencer with length,
envelope, sweep and linear-counter clocks. The mixer uses the documented
non-linear formula and downsamples the ~1.79 MHz output to 44.1 kHz using a
fractional cycle accumulator. DMC sample bytes are fetched from the bus on
request. Reading `$4015` acknowledges the frame IRQ; the DMC IRQ is included in
the CPU IRQ level.

The APU accumulates samples in a `Vec<f32>` that the emulator drains each frame.
The desktop app forwards them to the platform audio backend, which buffers them
in a mutex-protected queue consumed by the audio callback/thread.

## Audio backends

- Windows/macOS: `cpal` with an f32 stereo output stream. The callback pulls
  interleaved samples from the shared queue and outputs silence when empty.
- Linux: ALSA is opened through `libloading` at runtime (`libasound.so.2`), with
  `snd_pcm_set_params` requesting f32 interleaved stereo. A dedicated thread
  writes blocks with `snd_pcm_writei` and recovers from underruns. This keeps the
  build free of ALSA development headers. If the library or device is missing,
  `AudioOutput` reports the error and the emulator runs silently.

Both backends drop the oldest queued samples if the producer outruns the device.

## Save states

`src/save_state` serializes a `SaveState` document:

```json
{
  "version": 1,
  "rom_id": 123456789,
  "frame_count": 412,
  "created_at": 1760000000,
  "cpu": { "...": "..." },
  "bus": {
    "ram": [0, 0],
    "ppu": { "...": "..." },
    "apu": { "...": "..." },
    "controllers": [],
    "open_bus": 0
  }
}
```

The ROM fingerprint is an FNV-1a hash of PRG ROM, CHR ROM and mapper metadata.
Loading rejects mismatched fingerprints and unknown versions with an explicit
error. Mapper state, PRG RAM and CHR RAM are included; static ROM data is not
duplicated. Writes go to a temporary file and are renamed into place.

## Desktop application

`src/app` owns `Option<Session>`; a session contains the `Emulator`, input state,
frame pacing accumulator, FPS counter and a cached egui texture. Each update
applies keyboard state, runs zero or more emulated frames to match the NTSC
frame rate, and pushes audio. When paused, no frames run and the audio queue is
cleared to avoid stale sound.

Settings, library entries and achievement records are stored as JSON under the
platform configuration directory (for example `%APPDATA%\nes-core` on Windows).
Library artwork is read from a neighbouring `<rom>.png`/`.jpg` when present and
otherwise generated deterministically from the ROM fingerprint.

## Testing

- `tests/cpu.rs` exercises addressing, flags, cycle penalties, stack behaviour,
  interrupts and unofficial instructions with a RAM-only bus.
- `tests/cartridge.rs` builds synthetic iNES images and checks validation, NROM
  mirroring, bank switching for UxROM/CNROM/MMC1/MMC3/MMC5/VRC6, MMC5's
multiplier, PRG RAM and scanline IRQ, and CHR RAM behaviour.
- `tests/emulator.rs` builds an NROM image that programs the PPU, enables NMI and
  counts NMIs; it asserts rendered pixels, audio sample counts, controller
  shifting and save-state round-trips/version rejection.
