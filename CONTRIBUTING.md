# Contributing to NES Core

Thanks for taking a look. This is a small emulator project; the most valuable
contributions are precise bug reports with reproducible test ROMs, mapper
implementations, and fixes to CPU/PPU/APU accuracy.

## Ground rules

- **No copyrighted ROMs, BIOS files, artwork or audio.** Do not attach them to
  issues or pull requests, and do not add download links. Use homebrew or
  self-built test ROMs.
- Do not claim compatibility that has not been tested. Update the status table
  in the README only for behaviour you have actually verified.
- Keep modules separated: hardware code belongs under `src/<component>/` and
  must not depend on the GUI.

## Development setup

```sh
cargo build
cargo test --all-targets
cargo fmt
cargo clippy --all-targets -- -D warnings
```

The test suite builds its own small NROM image, so it runs without external ROM
files. CPU tests use a RAM-only test bus.

## Adding a mapper

1. Add a variant and state struct in `src/cartridge/mapper.rs`.
2. Implement register writes, PRG/CHR offset calculations in
   `src/cartridge/mod.rs`, and mirroring overrides if needed.
3. Add the mapper number to `SUPPORTED_MAPPERS` and the human-readable name.
4. Add tests in `tests/cartridge.rs` that construct a synthetic ROM.
5. Update the README status table and `docs/architecture.md`.

## Submitting changes

- Keep commits focused and use the imperative mood.
- Explain what was tested and what was not.
- Add or update tests for behaviour changes.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo test --all-targets` before opening a pull request.

## Reporting bugs

Include the NES Core version, operating system, the mapper number if known, and
a description of the failing behaviour. A legally distributable test ROM that
reproduces the problem is ideal. Never link to commercial ROM downloads.
