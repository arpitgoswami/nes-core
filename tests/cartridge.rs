use nes_core::cartridge::{Cartridge, Mirroring, RomError};

fn ines(prg: &[u8], chr: &[u8], mapper: u16, flags6: u8) -> Vec<u8> {
    assert_eq!(prg.len() % 16384, 0);
    assert_eq!(chr.len() % 8192, 0);
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

#[test]
fn rejects_bad_magic() {
    let mut rom = ines(&vec![0; 16384], &[], 0, 0);
    rom[0] = b'X';
    assert_eq!(Cartridge::from_bytes(&rom).unwrap_err(), RomError::BadMagic);
}

#[test]
fn rejects_truncated_prg() {
    let rom = ines(&vec![0; 16384], &[], 0, 0);
    let err = Cartridge::from_bytes(&rom[..rom.len() - 1]).unwrap_err();
    assert!(matches!(err, RomError::TruncatedPrg { .. }));
}

#[test]
fn rejects_unsupported_mapper() {
    let rom = ines(&vec![0; 16384], &[], 69, 0);
    assert_eq!(
        Cartridge::from_bytes(&rom).unwrap_err(),
        RomError::UnsupportedMapper(69)
    );
}

#[test]
fn nrom_maps_and_mirrors_16k_prg() {
    let mut prg = vec![0u8; 16384];
    prg[0] = 0x11;
    prg[0x3FFF] = 0x22;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 0, 0)).unwrap();
    assert_eq!(cart.cpu_read(0x8000), 0x11);
    assert_eq!(cart.cpu_read(0xBFFF), 0x22);
    assert_eq!(cart.cpu_read(0xC000), 0x11);
    assert_eq!(cart.cpu_read(0xFFFF), 0x22);
    assert!(cart.chr_is_ram());
}

#[test]
fn nametable_mirroring_is_applied() {
    let horizontal = Cartridge::from_bytes(&ines(&vec![0; 16384], &[], 0, 0)).unwrap();
    assert_eq!(horizontal.mirroring, Mirroring::Horizontal);
    assert_eq!(
        horizontal.nametable_index(0x2000),
        horizontal.nametable_index(0x2400)
    );
    assert_ne!(
        horizontal.nametable_index(0x2000),
        horizontal.nametable_index(0x2800)
    );

    let vertical = Cartridge::from_bytes(&ines(&vec![0; 16384], &[], 0, 1)).unwrap();
    assert_eq!(vertical.mirroring, Mirroring::Vertical);
    assert_ne!(
        vertical.nametable_index(0x2000),
        vertical.nametable_index(0x2400)
    );
    assert_eq!(
        vertical.nametable_index(0x2000),
        vertical.nametable_index(0x2800)
    );
}

#[test]
fn uxrom_switches_banks_and_fixes_the_last_one() {
    let mut prg = vec![0u8; 32768];
    prg[0] = 0x11;
    prg[0x4000] = 0x22;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 2, 0)).unwrap();
    assert_eq!(cart.cpu_read(0x8000), 0x11);
    assert_eq!(cart.cpu_read(0xC000), 0x22);
    cart.cpu_write(0x8000, 1);
    assert_eq!(cart.cpu_read(0x8000), 0x22);
    assert_eq!(cart.cpu_read(0xC000), 0x22);
}

#[test]
fn cnrom_switches_chr_banks() {
    let prg = vec![0u8; 32768];
    let mut chr = vec![0u8; 16384];
    chr[0] = 0xA1;
    chr[0x2000] = 0xB2;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 3, 0)).unwrap();
    assert_eq!(cart.chr_read(0), 0xA1);
    cart.cpu_write(0x8000, 1);
    assert_eq!(cart.chr_read(0), 0xB2);
}

#[test]
fn mmc1_shift_register_programs_banks() {
    let mut prg = vec![0u8; 32768];
    prg[0] = 0x11;
    prg[0x4000] = 0x22;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 1, 0)).unwrap();
    // Default control: PRG mode 3 (switch $8000, fix last $C000).
    for bit in [1, 0, 0, 0, 0] {
        cart.cpu_write(0xE000, bit);
    }
    assert_eq!(cart.cpu_read(0x8000), 0x22);
    assert_eq!(cart.cpu_read(0xC000), 0x22);
}

#[test]
fn mmc1_control_updates_mirroring() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 32768], &[], 1, 0)).unwrap();
    // Control value 0x0E: vertical mirroring, PRG mode 3.
    for bit in [0, 1, 1, 1, 0] {
        cart.cpu_write(0x8000, bit);
    }
    assert_eq!(cart.mirroring, Mirroring::Vertical);
}

#[test]
fn mmc3_switches_prg_and_raises_irq() {
    let mut prg = vec![0u8; 32768];
    prg[0] = 0x11;
    prg[0x2000] = 0x33;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 4, 0)).unwrap();
    // Register 7 selects the 8KB bank mapped at $C000 in the default mode.
    cart.cpu_write(0x8000, 7);
    cart.cpu_write(0x8001, 1);
    assert_eq!(cart.cpu_read(0xC000), 0x33);

    // Latch 2, reload, enable; the third scanline raises IRQ.
    cart.cpu_write(0xC000, 2);
    cart.cpu_write(0xC001, 0);
    cart.cpu_write(0xE001, 0);
    cart.on_scanline();
    assert!(!cart.irq_pending());
    cart.on_scanline();
    assert!(!cart.irq_pending());
    cart.on_scanline();
    assert!(cart.irq_pending());
    cart.cpu_write(0xE000, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn mmc5_switches_four_8k_prg_banks() {
    let mut prg = vec![0u8; 65536];
    prg[0x2000] = 0x33;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 5, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "MMC5 (5)");
    cart.cpu_write(0x5100, 3);
    cart.cpu_write(0x5114, 1);
    assert_eq!(cart.cpu_read(0x8000), 0x33);
}

#[test]
fn mmc5_banks_prg_ram_and_exposes_multiplier() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &[], 5, 0)).unwrap();
    cart.cpu_write(0x5113, 0);
    cart.cpu_write(0x6000, 0xAA);
    cart.cpu_write(0x5113, 1);
    assert_eq!(cart.cpu_read(0x6000), 0x00);
    cart.cpu_write(0x6000, 0xBB);
    cart.cpu_write(0x5113, 0);
    assert_eq!(cart.cpu_read(0x6000), 0xAA);

    cart.cpu_write(0x5205, 7);
    cart.cpu_write(0x5206, 9);
    assert_eq!(cart.cpu_read(0x5205), 63);
    assert_eq!(cart.cpu_read(0x5206), 0);

    cart.cpu_write(0x5104, 2);
    cart.cpu_write(0x5C00, 0x5A);
    assert_eq!(cart.cpu_read(0x5C00), 0x5A);
}

#[test]
fn mmc5_scanline_irq_fires_on_compare() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &[], 5, 0)).unwrap();
    cart.cpu_write(0x5203, 5);
    cart.cpu_write(0x5204, 0x80);
    for _ in 0..4 {
        cart.on_scanline();
    }
    assert!(!cart.irq_pending());
    cart.on_scanline();
    assert!(cart.irq_pending());
    assert_eq!(cart.cpu_read(0x5204) & 0x80, 0x80);
}

#[test]
fn mmc5_uses_sprite_and_background_chr_banks() {
    let mut chr = vec![0u8; 16384];
    chr[0x00] = 0x10;
    chr[0x400] = 0x20;
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &chr, 5, 0)).unwrap();
    cart.cpu_write(0x5101, 3);
    cart.cpu_write(0x5120, 1);
    cart.cpu_write(0x5128, 0);
    assert_eq!(cart.chr_read(0x0000), 0x10);
    assert_eq!(cart.chr_read_sprite(0x0000), 0x20);
}

#[test]
fn vrc6_switches_prg_and_chr_banks() {
    let mut prg = vec![0u8; 32768];
    prg[0x4000] = 0x44;
    let mut chr = vec![0u8; 8192];
    chr[0x400] = 0x99;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 24, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "VRC6a (24)");
    cart.cpu_write(0x8000, 1);
    assert_eq!(cart.cpu_read(0x8000), 0x44);
    cart.cpu_write(0xD001, 1);
    assert_eq!(cart.chr_read(0x0400), 0x99);
}

#[test]
fn vrc6_supports_mirroring_and_scanline_irq() {
    let mut cart =
        Cartridge::from_bytes(&ines(&vec![0u8; 32768], &vec![0u8; 8192], 24, 0)).unwrap();
    cart.cpu_write(0xB003, 0x2C);
    assert_eq!(cart.mirroring, Mirroring::OneScreenUpper);

    cart.cpu_write(0xF000, 3);
    cart.cpu_write(0xF001, 0x02);
    for _ in 0..4 {
        cart.on_scanline();
    }
    assert!(cart.irq_pending());
    cart.cpu_write(0xF002, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn vrc6b_swaps_register_address_lines() {
    let mut chr = vec![0u8; 8192];
    chr[0x400] = 0x77;
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 32768], &chr, 26, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "VRC6b (26)");
    // On VRC6b, $D002 addresses CHR register 1.
    cart.cpu_write(0xD002, 1);
    assert_eq!(cart.chr_read(0x0400), 0x77);
}

#[test]
fn chr_ram_writes_are_ignored_for_chr_rom() {
    let prg = vec![0u8; 16384];
    let mut chr = vec![0u8; 8192];
    chr[0x10] = 0x5A;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 0, 0)).unwrap();
    cart.chr_write(0x10, 0xFF);
    assert_eq!(cart.chr_read(0x10), 0x5A);
}

#[test]
fn mmc5_expansion_audio_outputs_pulses_and_pcm() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &vec![0u8; 8192], 5, 0)).unwrap();
    cart.cpu_write(0x5015, 0x01);
    cart.cpu_write(0x5000, 0xBF); // duty 50%, length halt, constant volume 15
    cart.cpu_write(0x5002, 0x64);
    cart.cpu_write(0x5003, 0x08);
    let mut peak = 0.0f32;
    for _ in 0..2000 {
        cart.clock_audio();
        peak = peak.max(cart.audio_output());
    }
    assert!(peak > 0.05, "pulse output too quiet: {peak}");
    assert_eq!(cart.cpu_read(0x5015) & 0x01, 0x01);

    cart.cpu_write(0x5015, 0x05);
    cart.cpu_write(0x5011, 0xFF);
    assert!(
        cart.audio_output() > 0.2,
        "the PCM DAC should be audible on top of the pulses"
    );

    cart.cpu_write(0x5015, 0x00);
    assert_eq!(cart.audio_output(), 0.0);
}

#[test]
fn mmc5_pcm_irq_is_acknowledged_by_reading_5010() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &vec![0u8; 8192], 5, 0)).unwrap();
    cart.cpu_write(0x5010, 0x80);
    cart.cpu_write(0x5011, 0x40);
    assert!(!cart.irq_pending());

    // A zero write keeps the DAC value and raises the PCM IRQ.
    cart.cpu_write(0x5011, 0x00);
    assert!(cart.irq_pending());
    assert_eq!(cart.cpu_read(0x5010) & 0x80, 0x80);
    assert!(!cart.irq_pending());
}

#[test]
fn mmc5_extended_attributes_select_palette_and_chr_bank() {
    let mut cart =
        Cartridge::from_bytes(&ines(&vec![0u8; 65536], &vec![0u8; 32768], 5, 0)).unwrap();
    cart.cpu_write(0x5104, 1);
    cart.cpu_write(0x5130, 1);
    cart.cpu_write(0x5C00, 0x03);
    cart.cpu_write(0x5C01, 0x41);

    let render = cart.mmc5_render().expect("MMC5 exposes background state");
    assert!(render.extended_attributes);
    assert!(!render.split_enabled);
    assert_eq!(render.extended_attribute(0, 0), (0, 3 | (1 << 6)));
    assert_eq!(render.extended_attribute(0, 1), (1, 1 | (1 << 6)));
}

#[test]
fn mmc5_split_screen_reads_exram_nametable() {
    let mut cart =
        Cartridge::from_bytes(&ines(&vec![0u8; 65536], &vec![0u8; 32768], 5, 0)).unwrap();
    cart.cpu_write(0x5104, 0);
    cart.cpu_write(0x5200, 0x80 | 16); // enabled, left side, threshold 16 tiles
    cart.cpu_write(0x5201, 0x08); // split scroll: second tile row
    cart.cpu_write(0x5202, 2); // 4 KB CHR bank 2
    cart.cpu_write(0x5C00 + 32 + 5, 0x42);
    cart.cpu_write(0x5C00 + 960 + 2 * 8 + 1, 0x03);

    let render = cart.mmc5_render().expect("MMC5 exposes background state");
    assert!(render.split_region(0));
    assert!(render.split_region(15));
    assert!(!render.split_region(16));
    assert_eq!(render.split_tile(0, 5), (0x42, 3));
    assert_eq!(render.split_fine_row(0), 0);
    assert_eq!(render.split_pattern_base(0x42), 2 * 0x1000 + 0x42 * 16);

    // RAM mode 2 disables the split screen.
    cart.cpu_write(0x5104, 2);
    let render = cart.mmc5_render().unwrap();
    assert!(!render.split_enabled);
}

#[test]
fn vrc6_expansion_audio_outputs_pulses_and_saw() {
    let mut cart =
        Cartridge::from_bytes(&ines(&vec![0u8; 32768], &vec![0u8; 8192], 24, 0)).unwrap();
    cart.cpu_write(0x9000, 0x8F); // ignore duty, full volume
    cart.cpu_write(0x9001, 0x40);
    cart.cpu_write(0x9002, 0x80);
    let mut peak = 0.0f32;
    for _ in 0..2000 {
        cart.clock_audio();
        peak = peak.max(cart.audio_output());
    }
    assert!(peak > 0.05, "VRC6 pulse output too quiet: {peak}");

    cart.cpu_write(0xB000, 0x20);
    cart.cpu_write(0xB001, 0x40);
    cart.cpu_write(0xB002, 0x80);
    let mut saw_peak = 0.0f32;
    for _ in 0..4000 {
        cart.clock_audio();
        saw_peak = saw_peak.max(cart.audio_output());
    }
    assert!(
        saw_peak > peak,
        "sawtooth should add to the pulse output ({saw_peak} vs {peak})"
    );

    // $9003 bit 0 halts every oscillator.
    cart.cpu_write(0x9003, 0x01);
    let halted = cart.audio_output();
    for _ in 0..1000 {
        cart.clock_audio();
    }
    assert_eq!(cart.audio_output(), halted);
}

#[test]
fn mmc2_switches_prg_and_latches_chr_banks() {
    let mut prg = vec![0u8; 128 * 1024];
    for bank in 0..16usize {
        prg[bank * 0x2000] = bank as u8 + 1;
    }
    let mut chr = vec![0u8; 32 * 1024];
    for bank in 0..8usize {
        chr[bank * 0x1000] = 0x80 | bank as u8;
    }
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 9, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "MMC2 (9)");

    // Bank 0 sits at $8000 and the last three 8 KB banks are fixed above it.
    assert_eq!(cart.cpu_read(0x8000), 1);
    assert_eq!(cart.cpu_read(0xA000), 14);
    assert_eq!(cart.cpu_read(0xE000), 16);
    cart.cpu_write(0xA000, 3);
    assert_eq!(cart.cpu_read(0x8000), 4);

    cart.cpu_write(0xB000, 1); // $FD bank for PPU $0000
    cart.cpu_write(0xC000, 2); // $FE bank for PPU $0000
    cart.cpu_write(0xD000, 3); // $FD bank for PPU $1000
    cart.cpu_write(0xE000, 4); // $FE bank for PPU $1000

    // The latch starts on $FE.
    assert_eq!(cart.chr_read(0x0000), 0x82);
    assert_eq!(cart.chr_read(0x1000), 0x84);

    // A PPU fetch of tile $FD's high bitplane flips latch 0 to $FD.
    cart.chr_read_ppu(0x0FD8, false);
    assert_eq!(cart.chr_read(0x0000), 0x81);
    cart.chr_read_ppu(0x1FD8, false);
    assert_eq!(cart.chr_read(0x1000), 0x83);

    cart.cpu_write(0xF000, 1);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
}

#[test]
fn mmc4_switches_16k_prg_banks_and_prg_ram() {
    let mut prg = vec![0u8; 128 * 1024];
    for bank in 0..16usize {
        prg[bank * 0x2000] = bank as u8 + 1;
    }
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 10, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "MMC4 (10)");

    assert_eq!(cart.cpu_read(0xC000), 15);
    cart.cpu_write(0xA000, 2);
    assert_eq!(cart.cpu_read(0x8000), 5);
    assert_eq!(cart.cpu_read(0xA000), 6);

    cart.cpu_write(0x6000, 0x77);
    cart.cpu_write(0xA001, 0);
    assert_eq!(cart.cpu_read(0x6000), 0);
    cart.cpu_write(0xA001, 1);
    assert_eq!(cart.cpu_read(0x6000), 0x77);
}

#[test]
fn color_dreams_switches_32k_prg_and_chr() {
    let mut prg = vec![0u8; 128 * 1024];
    for bank in 0..4usize {
        prg[bank * 0x8000] = 0x10 + bank as u8;
    }
    let mut chr = vec![0u8; 32 * 1024];
    for bank in 0..4usize {
        chr[bank * 0x2000] = 0x40 + bank as u8;
    }
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 11, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "Color Dreams (11)");
    cart.cpu_write(0x8000, 0x21);
    assert_eq!(cart.cpu_read(0x8000), 0x11);
    assert_eq!(cart.chr_read(0x0000), 0x42);
}

#[test]
fn gxrom_switches_prg_and_chr_banks() {
    let mut prg = vec![0u8; 128 * 1024];
    for bank in 0..4usize {
        prg[bank * 0x8000] = 0x50 + bank as u8;
    }
    let mut chr = vec![0u8; 32 * 1024];
    for bank in 0..4usize {
        chr[bank * 0x2000] = 0x60 + bank as u8;
    }
    let mut cart = Cartridge::from_bytes(&ines(&prg, &chr, 66, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "GxROM (66)");
    cart.cpu_write(0x8000, 0x12);
    assert_eq!(cart.cpu_read(0x8000), 0x51);
    assert_eq!(cart.chr_read(0x0000), 0x62);
}

#[test]
fn mmc5_maps_fill_mode_and_exram_nametables() {
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 65536], &vec![0u8; 8192], 5, 0)).unwrap();
    cart.cpu_write(0x5104, 0);
    cart.cpu_write(0x5105, 0xAA); // every table from ExRAM
    cart.cpu_write(0x5C00, 0x5A);
    cart.cpu_write(0x5C01, 0x3C);
    assert_eq!(cart.nametable_read(0x2000), Some(0x5A));
    assert!(cart.nametable_write(0x2001, 0x77));
    assert_eq!(cart.nametable_read(0x2401), Some(0x77));
    // $5C00 is write-only in modes 0 and 1.
    assert_eq!(cart.cpu_read(0x5C00), 0);

    // In modes 2 and 3 ExRAM is not a nametable and reads as zero.
    cart.cpu_write(0x5104, 2);
    assert_eq!(cart.nametable_read(0x2000), Some(0));
    assert!(cart.nametable_write(0x2000, 0x11));
    assert_eq!(cart.cpu_read(0x5C00), 0x5A);

    cart.cpu_write(0x5104, 0);
    cart.cpu_write(0x5105, 0xFF);
    cart.cpu_write(0x5106, 0x2A);
    cart.cpu_write(0x5107, 0x02);
    assert_eq!(cart.nametable_read(0x2000), Some(0x2A));
    assert_eq!(cart.nametable_read(0x23C0), Some(0xAA));
    assert!(cart.nametable_write(0x2000, 0x00));
    assert_eq!(cart.nametable_read(0x2000), Some(0x2A));
}

#[test]
fn mmc5_pcm_read_mode_feeds_prg_bytes_to_the_dac() {
    let mut prg = vec![0u8; 65536];
    prg[0] = 0x40;
    prg[1] = 0xC0;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &vec![0u8; 8192], 5, 0)).unwrap();
    cart.cpu_write(0x5015, 0x04);
    cart.cpu_write(0x5010, 0x01); // read mode
    assert_eq!(cart.cpu_read(0x8000), 0x40);
    assert!(
        cart.audio_output() < 0.0,
        "the DAC should follow the PRG byte"
    );
    cart.cpu_read(0x8001);
    assert!(cart.audio_output() > 0.0);
    // Writes to $5011 are ignored while read mode is active.
    cart.cpu_write(0x5011, 0xFF);
    assert!(cart.audio_output() > 0.0);
}

#[test]
fn vrc6_switchable_chr_banking_modes() {
    let mut chr = vec![0u8; 64 * 1024];
    for bank in 0..16usize {
        chr[bank * 0x400] = 0x10 + bank as u8;
    }
    let mut cart = Cartridge::from_bytes(&ines(&vec![0u8; 32768], &chr, 24, 0)).unwrap();
    // 2 KB mode with bit 5 set: registers 0-3 feed pairs of 1 KB slots.
    cart.cpu_write(0xB003, 0x21);
    cart.cpu_write(0xD000, 2);
    cart.cpu_write(0xD001, 4);
    cart.cpu_write(0xD002, 6);
    cart.cpu_write(0xD003, 8);
    assert_eq!(cart.chr_read(0x0000), 0x12);
    assert_eq!(cart.chr_read(0x0400), 0x13);
    assert_eq!(cart.chr_read(0x0800), 0x14);
    assert_eq!(cart.chr_read(0x0C00), 0x15);
    assert_eq!(cart.chr_read(0x1000), 0x16);
    assert_eq!(cart.chr_read(0x1400), 0x17);

    // 4 KB mode: slots 0-3 use registers 0-3 directly, slots 4-7 pair 4 and 5.
    cart.cpu_write(0xB003, 0x22);
    assert_eq!(cart.chr_read(0x0400), 0x14);
    cart.cpu_write(0xE000, 10);
    cart.cpu_write(0xE001, 10);
    assert_eq!(cart.chr_read(0x1000), 0x1A);
    assert_eq!(cart.chr_read(0x1800), 0x1A);
}

#[test]
fn vrc6_cycle_irq_counts_cpu_cycles() {
    let mut cart =
        Cartridge::from_bytes(&ines(&vec![0u8; 32768], &vec![0u8; 8192], 24, 0)).unwrap();
    cart.cpu_write(0xF000, 3);
    cart.cpu_write(0xF001, 0x06); // cycle mode, enabled
    assert!(!cart.irq_pending());
    for _ in 0..3 {
        cart.clock_cpu();
    }
    assert!(!cart.irq_pending());
    cart.clock_cpu();
    assert!(cart.irq_pending());
    cart.cpu_write(0xF002, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn quattro_switches_outer_and_inner_banks() {
    let mut prg = vec![0u8; 256 * 1024];
    prg[0] = 0xA1;
    prg[0x10000] = 0xB2;
    prg[0x4000] = 0xC3;
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 232, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "Quattro (232)");
    assert_eq!(cart.cpu_read(0x8000), 0xA1);
    cart.cpu_write(0x8000, 0x08); // outer 64 KB bank 1
    assert_eq!(cart.cpu_read(0x8000), 0xB2);
    cart.cpu_write(0xC000, 1);
    assert_eq!(cart.cpu_read(0xC000), 0xC3);
}

#[test]
fn camerica_switches_16k_banks_and_fire_hawk_mirroring() {
    let mut prg = vec![0u8; 64 * 1024];
    for bank in 0..4usize {
        prg[bank * 0x4000] = 0x20 + bank as u8;
    }
    let mut cart = Cartridge::from_bytes(&ines(&prg, &[], 71, 0)).unwrap();
    assert_eq!(cart.mapper_name(), "Camerica (71)");

    // $8000 holds the last 16 KB bank; $C000 is switchable.
    assert_eq!(cart.cpu_read(0x8000), 0x23);
    cart.cpu_write(0xC000, 1);
    assert_eq!(cart.cpu_read(0xC000), 0x21);

    cart.cpu_write(0x9000, 0x10);
    assert_eq!(cart.mirroring, Mirroring::OneScreenUpper);
}
