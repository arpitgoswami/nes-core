# Building NES Core

## Prerequisites

- Rust stable 1.80 or newer (`rustup` recommended).
- A C toolchain is only needed for crates that build C code on your platform;
  NES Core itself contains no C.
- Windows: MSVC build tools ("Desktop development with C++"). The GNU toolchain
  is not tested.
- Linux: X11 or Wayland runtime libraries and a Vulkan/OpenGL driver. Audio uses
  ALSA at runtime and is loaded dynamically, so `libasound2-dev` is **not**
  required to build.
- macOS: Xcode command line tools. Audio uses cpal/CoreAudio.

## Debug build

```sh
cargo build
cargo run
```

## Release build

```sh
cargo build --release
```

The binary is written to:

- Windows: `target\release\nes-core.exe`
- Linux: `target/release/nes-core`
- macOS: `target/release/nes-core`

Debug and release builds use the wgpu renderer. `Cargo.toml` sets thin LTO,
stripped symbols and a single codegen unit for release builds.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

The test suite needs no ROM files. It constructs synthetic NROM images and uses
a RAM-only bus for CPU tests. The audio tests only inspect generated samples;
they do not require an audio device.

## Verifying another target

Type-checking a platform without a native linker is possible with `cargo check`:

```sh
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc
```

This compiles the Windows-specific cpal audio path. The Windows CI job performs a
real release build and uploads `nes-core.exe` as a workflow artifact.

## Running headless

NES Core opens a native window and needs a GPU/windowing session. There is no
standard headless mode. On a server without a display, use the library tests to
validate emulation; starting the GUI requires X11/Wayland (or a virtual display)
plus a wgpu-compatible adapter.

## Audio notes

- If no audio device is available, the app shows "No audio device (silent)" in
  the sidebar and continues running. The emulator core is unaffected.
- Linux: the ALSA shared library (`libasound.so.2`) must be present at runtime.
  This is standard on desktop distributions. Set `ALSA_CARD` or configure the
  default PCM through `/etc/asound.conf` or `~/.asoundrc` if the default device
  is not the one you want.
- Windows/macOS: the default output device is used; the sample rate is 44.1 kHz
  stereo f32.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| "No audio device (silent)" | No default output device, or ALSA library missing on Linux |
| Window opens but the emulator view is black before loading a ROM | Expected: there is no ROM loaded |
| "mapper N is not supported by this build" | The ROM uses a mapper outside 0/1/2/3/4/7 |
| "missing iNES signature" | The file is not an iNES ROM (or is a raw/headered dump of another format) |
| "PRG data is truncated" | The file is incomplete or the header is wrong |
| Save state fails to load | State from a different ROM or a newer format version |
