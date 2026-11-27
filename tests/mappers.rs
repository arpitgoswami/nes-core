use nes_core::cartridge::{Cartridge, Mirroring};

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

/// PRG image where every 8 KB bank starts with its own bank number + 1.
fn prg_marked(banks_8k: usize) -> Vec<u8> {
    let mut prg = vec![0u8; banks_8k * 0x2000];
    for bank in 0..banks_8k {
        prg[bank * 0x2000] = bank as u8 + 1;
    }
    prg
}

/// CHR image where every 1 KB bank starts with its own bank number + 1.
fn chr_marked(banks_1k: usize) -> Vec<u8> {
    let mut chr = vec![0u8; banks_1k * 0x400];
    for bank in 0..banks_1k {
        chr[bank * 0x400] = bank as u8 + 1;
    }
    chr
}

fn load(mapper: u16, prg_8k: usize, chr_1k: usize) -> Cartridge {
    Cartridge::from_bytes(&ines(&prg_marked(prg_8k), &chr_marked(chr_1k), mapper, 0)).unwrap()
}

fn prg(cart: &mut Cartridge) -> [u8; 4] {
    [
        cart.cpu_read(0x8000),
        cart.cpu_read(0xA000),
        cart.cpu_read(0xC000),
        cart.cpu_read(0xE000),
    ]
}

fn chr(cart: &Cartridge) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = cart.chr_read(i as u16 * 0x400);
    }
    out
}

#[test]
fn every_listed_mapper_loads() {
    for mapper in [
        0u16, 1, 2, 3, 4, 5, 7, 9, 10, 11, 16, 21, 22, 23, 24, 25, 26, 33, 34, 66, 69, 70, 71, 75,
        78, 79, 85, 87, 88, 89, 93, 94, 113, 140, 146, 152, 154, 159, 180, 184, 206, 232,
    ] {
        let cart = Cartridge::from_bytes(&ines(&prg_marked(16), &chr_marked(32), mapper, 0));
        assert!(cart.is_ok(), "mapper {mapper} should load");
    }
}

#[test]
fn mmc1_control_register_keeps_all_five_bits() {
    let mut cart = load(1, 16, 16);
    // Control = 0b00000: one-screen lower, 32 KB PRG, 8 KB CHR mode. Before the
    // fix the fifth bit was forced to 1, which silently selected 4 KB CHR mode.
    for _ in 0..5 {
        cart.cpu_write(0x8000, 0);
    }
    assert_eq!(cart.mirroring, Mirroring::OneScreenLower);
    // CHR bank register 3 in 8 KB mode ignores bit 0 and maps 8 KB bank 1.
    for bit in [1, 1, 0, 0, 0] {
        cart.cpu_write(0xA000, bit);
    }
    assert_eq!(chr(&cart), [9, 10, 11, 12, 13, 14, 15, 16]);
}

#[test]
fn mmc3_prg_and_chr_banking_follows_hardware() {
    let mut cart = load(4, 16, 32);
    let write = |cart: &mut Cartridge, reg: u8, value: u8| {
        cart.cpu_write(0x8000, reg);
        cart.cpu_write(0x8001, value);
    };
    write(&mut cart, 6, 3);
    write(&mut cart, 7, 5);
    // $8000 = R6, $A000 = R7, $C000 = second-last, $E000 = last.
    assert_eq!(prg(&mut cart), [4, 6, 15, 16]);
    // PRG mode 1 swaps $8000 and $C000.
    cart.cpu_write(0x8000, 0x46);
    assert_eq!(prg(&mut cart), [15, 6, 4, 16]);
    cart.cpu_write(0x8000, 0x06);

    write(&mut cart, 0, 4);
    write(&mut cart, 1, 8);
    write(&mut cart, 2, 20);
    write(&mut cart, 3, 21);
    write(&mut cart, 4, 22);
    write(&mut cart, 5, 23);
    assert_eq!(chr(&cart), [5, 6, 9, 10, 21, 22, 23, 24]);
    // CHR inversion swaps the two halves.
    cart.cpu_write(0x8000, 0x80);
    assert_eq!(chr(&cart), [21, 22, 23, 24, 5, 6, 9, 10]);
}

#[test]
fn bnrom_switches_32k_prg() {
    let mut cart = Cartridge::from_bytes(&ines(&prg_marked(16), &[], 34, 0)).unwrap();
    cart.cpu_write(0x8000, 2);
    assert_eq!(prg(&mut cart), [9, 10, 11, 12]);
}

#[test]
fn nina001_registers_live_in_prg_ram_space() {
    let mut cart = load(34, 16, 32);
    cart.cpu_write(0x7FFD, 1);
    cart.cpu_write(0x7FFE, 2);
    cart.cpu_write(0x7FFF, 3);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart), [9, 10, 11, 12, 13, 14, 15, 16]);
}

#[test]
fn discrete_latch_mappers_bank_prg_and_chr() {
    let mut cart = load(70, 16, 64);
    cart.cpu_write(0x8000, 0x23);
    assert_eq!(prg(&mut cart), [5, 6, 15, 16]);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(152, 16, 64);
    cart.cpu_write(0x8000, 0x92);
    assert_eq!(prg(&mut cart)[0], 3);
    assert_eq!(cart.mirroring, Mirroring::OneScreenUpper);

    let mut cart = load(180, 16, 8);
    assert_eq!(prg(&mut cart), [1, 2, 1, 2]);
    cart.cpu_write(0x8000, 3);
    assert_eq!(prg(&mut cart), [1, 2, 7, 8]);

    let mut cart = load(94, 16, 8);
    cart.cpu_write(0x8000, 3 << 2);
    assert_eq!(prg(&mut cart), [7, 8, 15, 16]);
}

#[test]
fn nina03_and_sachen_use_the_4100_window() {
    let mut cart = load(79, 8, 64);
    cart.cpu_write(0x4100, 0x0B);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(113, 32, 128);
    cart.cpu_write(0x4100, 0x80 | 0x10 | 0x03);
    assert_eq!(prg(&mut cart)[0], 9);
    assert_eq!(cart.mirroring, Mirroring::Vertical);
}

#[test]
fn jaleco_and_sunsoft_register_windows() {
    let mut cart = load(87, 4, 32);
    cart.cpu_write(0x6000, 0x02);
    assert_eq!(chr(&cart)[0], 9);

    let mut cart = load(140, 16, 64);
    cart.cpu_write(0x6000, 0x12);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 17);

    let mut cart = load(184, 4, 64);
    cart.cpu_write(0x6000, 0x31);
    assert_eq!(chr(&cart)[0], 5);
    assert_eq!(chr(&cart)[4], 29);
}

#[test]
fn dxrom_has_fixed_prg_and_two_kb_chr() {
    let mut cart = load(206, 16, 64);
    let write = |cart: &mut Cartridge, reg: u8, value: u8| {
        cart.cpu_write(0x8000, reg);
        cart.cpu_write(0x8001, value);
    };
    write(&mut cart, 6, 2);
    write(&mut cart, 7, 3);
    write(&mut cart, 0, 4);
    write(&mut cart, 2, 9);
    assert_eq!(prg(&mut cart), [3, 4, 15, 16]);
    assert_eq!(&chr(&cart)[0..2], &[5, 6]);
    assert_eq!(chr(&cart)[4], 10);
}

#[test]
fn taito_tc0190_banks_prg_and_chr() {
    let mut cart = load(33, 16, 64);
    cart.cpu_write(0x8000, 0x45);
    cart.cpu_write(0x8001, 0x02);
    cart.cpu_write(0x8002, 3);
    cart.cpu_write(0xA001, 40);
    assert_eq!(prg(&mut cart), [6, 3, 15, 16]);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
    assert_eq!(&chr(&cart)[0..2], &[7, 8]);
    assert_eq!(chr(&cart)[5], 41);
}

#[test]
fn vrc1_banks_use_split_chr_registers() {
    let mut cart = load(75, 16, 128);
    cart.cpu_write(0x8000, 2);
    cart.cpu_write(0xA000, 3);
    cart.cpu_write(0xC000, 4);
    cart.cpu_write(0x9000, 0x02);
    cart.cpu_write(0xE000, 1);
    assert_eq!(prg(&mut cart), [3, 4, 5, 16]);
    // High bit of CHR0 set (16) plus low nibble 1 = bank 17 at 4 KB granularity.
    assert_eq!(chr(&cart)[0], 17 * 4 + 1);
}

#[test]
fn vrc4_decodes_registers_and_irq() {
    // Mapper 25 (VRC4b/d): A1 -> register bit 0, A0 -> register bit 1.
    let mut cart = load(25, 32, 128);
    cart.cpu_write(0x8000, 4);
    cart.cpu_write(0xA000, 5);
    assert_eq!(prg(&mut cart), [5, 6, 31, 32]);
    // $B000 low nibble of CHR0, $B002 (A1 set) high bits of CHR0.
    cart.cpu_write(0xB000, 0x03);
    cart.cpu_write(0xB002, 0x01);
    assert_eq!(chr(&cart)[0], 0x13 + 1);
    // PRG swap mode via $9002 moves the switchable bank to $C000.
    cart.cpu_write(0x9001, 2);
    assert_eq!(prg(&mut cart), [31, 6, 5, 32]);

    // Scanline IRQ: latch 0xFE then enable; two clocks reach 0xFF then overflow.
    cart.cpu_write(0xF000, 0x0E);
    cart.cpu_write(0xF002, 0x0F);
    cart.cpu_write(0xF001, 0x02);
    cart.on_scanline();
    assert!(!cart.irq_pending());
    cart.on_scanline();
    assert!(cart.irq_pending());
    cart.cpu_write(0xF003, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn fme7_banks_and_cycle_irq() {
    let mut cart = load(69, 16, 64);
    let write = |cart: &mut Cartridge, cmd: u8, value: u8| {
        cart.cpu_write(0x8000, cmd);
        cart.cpu_write(0xA000, value);
    };
    write(&mut cart, 9, 3);
    write(&mut cart, 10, 4);
    write(&mut cart, 11, 5);
    write(&mut cart, 2, 40);
    write(&mut cart, 12, 1);
    assert_eq!(prg(&mut cart), [4, 5, 6, 16]);
    assert_eq!(chr(&cart)[2], 41);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);

    write(&mut cart, 14, 3);
    write(&mut cart, 15, 0);
    write(&mut cart, 13, 0x81);
    for _ in 0..3 {
        cart.clock_cpu();
        assert!(!cart.irq_pending());
    }
    cart.clock_cpu();
    assert!(cart.irq_pending());
}

#[test]
fn bandai_fcg_banks_and_counts_cycles() {
    let mut cart = load(16, 16, 64);
    cart.cpu_write(0x8000, 7);
    cart.cpu_write(0x8008, 2);
    cart.cpu_write(0x8009, 0);
    assert_eq!(chr(&cart)[0], 8);
    assert_eq!(prg(&mut cart), [5, 6, 15, 16]);
    assert_eq!(cart.mirroring, Mirroring::Vertical);
    cart.cpu_write(0x800B, 2);
    cart.cpu_write(0x800C, 0);
    cart.cpu_write(0x800A, 1);
    cart.clock_cpu();
    assert!(!cart.irq_pending());
    cart.clock_cpu();
    assert!(cart.irq_pending());
}

#[test]
fn vrc7_banks_prg_chr_and_mirroring() {
    let mut cart = load(85, 32, 128);
    cart.cpu_write(0x8000, 1);
    cart.cpu_write(0x8010, 2);
    cart.cpu_write(0x9000, 3);
    cart.cpu_write(0xA000, 50);
    cart.cpu_write(0xA010, 51);
    cart.cpu_write(0xE000, 1);
    assert_eq!(prg(&mut cart), [2, 3, 4, 32]);
    assert_eq!(&chr(&cart)[0..2], &[51, 52]);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
}

#[test]
fn mapper_state_survives_a_round_trip() {
    let mut cart = load(69, 16, 64);
    cart.cpu_write(0x8000, 9);
    cart.cpu_write(0xA000, 6);
    let saved = cart.save_state();
    cart.cpu_write(0x8000, 9);
    cart.cpu_write(0xA000, 1);
    assert_eq!(prg(&mut cart)[0], 2);
    cart.load_state(&saved);
    assert_eq!(prg(&mut cart)[0], 7);
}
