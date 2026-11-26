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
    let rom = ines(&vec![0; 16384], &[], 66, 0);
    assert_eq!(
        Cartridge::from_bytes(&rom).unwrap_err(),
        RomError::UnsupportedMapper(66)
    );
}

#[test]
fn nrom_maps_and_mirrors_16k_prg() {
    let mut prg = vec![0u8; 16384];
    prg[0] = 0x11;
    prg[0x3FFF] = 0x22;
    let cart = Cartridge::from_bytes(&ines(&prg, &[], 0, 0)).unwrap();
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
