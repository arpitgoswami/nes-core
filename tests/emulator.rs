use nes_core::cartridge::Cartridge;
use nes_core::emulator::Emulator;
use nes_core::ppu::palette::PALETTE_RGB;
use nes_core::save_state;

fn ines(prg: &[u8], chr: &[u8], mapper: u16, flags6: u8) -> Vec<u8> {
    let mut rom = vec![
        b'N',
        b'E',
        b'S',
        0x1A,
        (prg.len() / 16384) as u8,
        (chr.len() / 8192) as u8,
        flags6 | (((mapper & 0x0F) as u8) << 4),
        (mapper & 0xF0) as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    rom.extend_from_slice(prg);
    rom.extend_from_slice(chr);
    rom
}

/// Builds a small NROM image that sets a palette, draws tile 1 at the top-left,
/// enables background rendering with NMI, and counts NMIs in $0300.
fn test_rom() -> Cartridge {
    let mut prg = Vec::new();
    prg.extend_from_slice(&[0x78, 0xD8, 0xA2, 0xFF, 0x9A]); // SEI CLD LDX #$FF TXS
    prg.extend_from_slice(&[0x2C, 0x02, 0x20, 0x10, 0xFB]); // BIT $2002; BPL
    prg.extend_from_slice(&[0xA9, 0x3F, 0x8D, 0x06, 0x20, 0xA9, 0x00, 0x8D, 0x06, 0x20]);
    prg.extend_from_slice(&[0xA9, 0x0F, 0x8D, 0x07, 0x20]);
    prg.extend_from_slice(&[0xA9, 0x21, 0x8D, 0x07, 0x20]);
    prg.extend_from_slice(&[0xA9, 0x20, 0x8D, 0x06, 0x20, 0xA9, 0x00, 0x8D, 0x06, 0x20]);
    prg.extend_from_slice(&[0xA9, 0x01, 0x8D, 0x07, 0x20]);
    // Reset the scroll/address so rendering starts at the top-left tile.
    prg.extend_from_slice(&[0xA9, 0x00, 0x8D, 0x06, 0x20, 0x8D, 0x06, 0x20]);
    prg.extend_from_slice(&[0xA9, 0x0A, 0x8D, 0x01, 0x20]); // background + left clip
    prg.extend_from_slice(&[0xA9, 0x80, 0x8D, 0x00, 0x20]); // NMI on
    assert_eq!(prg.len(), 63);
    prg.extend_from_slice(&[0x4C, 0x3F, 0x80]); // JMP to itself at $803F
    prg.resize(0x100, 0xEA);
    // NMI: reset the PPU address scroll, count the NMI, return.
    prg.extend_from_slice(&[
        0xA9, 0x00, 0x8D, 0x06, 0x20, 0x8D, 0x06, 0x20, 0xEE, 0x00, 0x03, 0x40,
    ]);
    prg.resize(0x4000, 0xEA);
    // Vectors live at the end of the 16KB PRG bank.
    prg[0x3FFA] = 0x00;
    prg[0x3FFB] = 0x81;
    prg[0x3FFC] = 0x00;
    prg[0x3FFD] = 0x80;
    prg[0x3FFE] = 0x00;
    prg[0x3FFF] = 0x81;

    let mut chr = vec![0u8; 8192];
    chr[16..24].fill(0xFF); // tile 1, low bitplane set -> colour 1

    Cartridge::from_bytes(&ines(&prg, &chr, 0, 0)).expect("synthetic ROM should parse")
}

#[test]
fn emulator_renders_and_handles_nmi() {
    let mut emulator = Emulator::new(test_rom());
    assert!(emulator.run_frame(), "frame should complete");
    assert!(emulator.run_frame(), "frame should complete");
    assert_eq!(emulator.frame_count, 2);

    let fb = emulator.framebuffer();
    let pixel = |x: usize, y: usize| {
        let o = (y * 256 + x) * 4;
        [fb[o], fb[o + 1], fb[o + 2], fb[o + 3]]
    };
    let tile_color = PALETTE_RGB[0x21];
    let backdrop = PALETTE_RGB[0x0F];
    assert_eq!(pixel(0, 0), [tile_color.0, tile_color.1, tile_color.2, 255]);
    assert_eq!(pixel(100, 100), [backdrop.0, backdrop.1, backdrop.2, 255]);

    for _ in 0..5 {
        assert!(emulator.run_frame());
    }
    assert!(
        emulator.bus.ram[0x300] >= 2,
        "NMI handler should have incremented its counter"
    );
}

#[test]
fn audio_samples_are_produced() {
    let mut emulator = Emulator::new(test_rom());
    emulator.run_frame();
    let mut samples = Vec::new();
    emulator.drain_audio(&mut samples);
    // 1.79 MHz / 44100 ≈ 40.5 CPU cycles per sample, ~735 samples per frame.
    assert!(
        samples.len() > 600 && samples.len() < 900,
        "unexpected sample count {}",
        samples.len()
    );
}

#[test]
fn save_state_round_trip_and_rom_check() {
    let mut emulator = Emulator::new(test_rom());
    emulator.run_frame();
    emulator.run_frame();

    let dir = std::env::temp_dir().join(format!("nes-core-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slot.json");
    save_state::save_to_path(&emulator, &path).unwrap();

    let saved_cycles = emulator.cpu.cycles;
    let saved_frames = emulator.frame_count;
    for _ in 0..3 {
        emulator.run_frame();
    }
    assert_ne!(emulator.cpu.cycles, saved_cycles);

    save_state::load_into(&mut emulator, &path).unwrap();
    assert_eq!(emulator.cpu.cycles, saved_cycles);
    assert_eq!(emulator.frame_count, saved_frames);

    let meta = save_state::read_meta(&path).unwrap();
    assert_eq!(meta.version, save_state::SAVE_STATE_VERSION);
    assert_eq!(meta.rom_id, emulator.rom_id);

    // A truncated or future-versioned state must not be applied.
    let text = std::fs::read_to_string(&path).unwrap();
    let broken = text.replace(
        "\"version\":1",
        &format!("\"version\":{}", save_state::SAVE_STATE_VERSION + 1),
    );
    std::fs::write(&path, broken).unwrap();
    let err = save_state::load_into(&mut emulator, &path).unwrap_err();
    assert!(matches!(
        err,
        save_state::SaveStateError::UnsupportedVersion { .. }
    ));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn controller_shift_register_reports_buttons() {
    use nes_core::cpu::CpuBus;
    let mut emulator = Emulator::new(test_rom());
    use nes_core::controller::Button;
    emulator.set_button(0, Button::A, true);
    emulator.set_button(0, Button::Start, true);
    emulator.bus.write(0x4016, 1);
    emulator.bus.write(0x4016, 0);
    let mut pressed = Vec::new();
    for _ in 0..8 {
        pressed.push(emulator.bus.read(0x4016));
    }
    // Order: A, B, Select, Start, Up, Down, Left, Right.
    assert_eq!(pressed, vec![1, 0, 0, 1, 0, 0, 0, 0]);
}

#[test]
fn pal_images_use_pal_ppu_timing() {
    let mut rom = ines(&vec![0u8; 16384], &[], 0, 0);
    rom[9] = 0x01;
    let cart = Cartridge::from_bytes(&rom).unwrap();
    assert!(cart.header.pal);
    let emu = Emulator::new(cart);
    assert_eq!(emu.bus.ppu.total_scanlines(), 312);
    assert_eq!(emu.bus.ppu.state.scanline, 311);
    assert!((emu.frame_rate() - 50.006_993).abs() < 1e-6);
}
