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
fn second_batch_of_mappers_loads() {
    for mapper in [
        13u16, 18, 32, 38, 48, 65, 67, 72, 73, 76, 86, 92, 95, 107, 112, 118, 133, 148, 149, 151,
        200, 201, 202, 203, 210, 225, 229, 240, 241, 242, 246,
    ] {
        let cart = Cartridge::from_bytes(&ines(&prg_marked(16), &chr_marked(32), mapper, 0));
        assert!(cart.is_ok(), "mapper {mapper} should load");
    }
}

#[test]
fn cprom_switches_four_chr_ram_pages() {
    let mut cart = Cartridge::from_bytes(&ines(&prg_marked(4), &[], 13, 0)).unwrap();
    cart.cpu_write(0x8000, 2);
    cart.chr_write(0x1000, 0x77);
    cart.cpu_write(0x8000, 0);
    cart.chr_write(0x1000, 0x11);
    assert_eq!(cart.chr_read(0x1000), 0x11);
    cart.cpu_write(0x8000, 2);
    assert_eq!(cart.chr_read(0x1000), 0x77);
}

#[test]
fn small_discrete_mappers_decode_their_registers() {
    let mut cart = load(38, 16, 32);
    cart.cpu_write(0x7000, 0b1101);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(86, 16, 64);
    cart.cpu_write(0x6000, 0x35);
    assert_eq!(prg(&mut cart)[0], 13);
    assert_eq!(chr(&cart)[0], 9);

    let mut cart = load(107, 16, 64);
    cart.cpu_write(0x8000, 0x06);
    assert_eq!(prg(&mut cart)[0], 13);
    assert_eq!(chr(&cart)[0], 49);

    let mut cart = load(148, 16, 64);
    cart.cpu_write(0x8000, 0x0B);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(149, 16, 64);
    cart.cpu_write(0x8000, 0x80);
    assert_eq!(chr(&cart)[0], 9);

    let mut cart = load(133, 16, 64);
    cart.cpu_write(0x4100, 0x07);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(240, 16, 64);
    cart.cpu_write(0x5000, 0x13);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 25);

    let mut cart = load(241, 16, 8);
    cart.cpu_write(0x8000, 2);
    assert_eq!(prg(&mut cart)[0], 9);

    let mut cart = load(242, 16, 8);
    cart.cpu_write(0x8000 | (3 << 3), 0);
    assert_eq!(prg(&mut cart)[0], 13);
}

#[test]
fn jaleco_jf17_and_jf19_latch_on_rising_bits() {
    let mut cart = load(72, 16, 64);
    cart.cpu_write(0x8000, 0x80 | 0x03);
    assert_eq!(prg(&mut cart), [7, 8, 15, 16]);
    // Writing again with the PRG bit still high does not re-latch.
    cart.cpu_write(0x8000, 0x80 | 0x05);
    assert_eq!(prg(&mut cart)[0], 7);
    cart.cpu_write(0x8000, 0x40 | 0x02);
    assert_eq!(chr(&cart)[0], 17);

    let mut cart = load(92, 16, 64);
    cart.cpu_write(0x8000, 0x80 | 0x02);
    assert_eq!(prg(&mut cart), [1, 2, 5, 6]);
}

#[test]
fn address_latch_multicarts() {
    let mut cart = load(200, 32, 64);
    cart.cpu_write(0x8000 | 0x8 | 0x3, 0);
    assert_eq!(prg(&mut cart), [7, 8, 7, 8]);
    assert_eq!(chr(&cart)[0], 25);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);

    let mut cart = load(201, 32, 64);
    cart.cpu_write(0x8001, 0);
    assert_eq!(prg(&mut cart)[0], 5);
    assert_eq!(chr(&cart)[0], 9);

    let mut cart = load(203, 32, 64);
    cart.cpu_write(0x8000, 0b0000_1101);
    assert_eq!(prg(&mut cart), [7, 8, 7, 8]);
    assert_eq!(chr(&cart)[0], 9);

    let mut cart = load(202, 32, 64);
    cart.cpu_write(0x8000 | (3 << 1), 0);
    assert_eq!(prg(&mut cart), [7, 8, 7, 8]);
    // Address bits 2 and 3 both set select 32 KB mode: bank 6 -> pair 3.
    cart.cpu_write(0x8000 | (6 << 1), 0);
    assert_eq!(prg(&mut cart), [13, 14, 15, 16]);

    let mut cart = load(225, 64, 128);
    // 16 KB mode (bit 12): bank 3 mirrored into both halves.
    cart.cpu_write(0x8000 | 0x1000 | (3 << 6) | 0x2000 | 5, 0);
    assert_eq!(prg(&mut cart), [7, 8, 7, 8]);
    assert_eq!(chr(&cart)[0], 41);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
    // 32 KB mode: bank 3 rounds down to the pair starting at bank 2.
    cart.cpu_write(0x8000 | (3 << 6), 0);
    assert_eq!(prg(&mut cart), [5, 6, 7, 8]);

    let mut cart = load(229, 64, 128);
    cart.cpu_write(0x8000, 0);
    assert_eq!(prg(&mut cart), [1, 2, 3, 4]);
    cart.cpu_write(0x8000 | 0x20 | 5, 0);
    assert_eq!(prg(&mut cart), [11, 12, 11, 12]);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
}

#[test]
fn fong_shen_bang_has_four_prg_and_four_chr_registers() {
    let mut cart = load(246, 16, 64);
    for (i, value) in [3u8, 4, 5, 6].into_iter().enumerate() {
        cart.cpu_write(0x6000 + i as u16, value);
    }
    cart.cpu_write(0x6004, 5);
    assert_eq!(prg(&mut cart), [4, 5, 6, 7]);
    assert_eq!(&chr(&cart)[0..2], &[11, 12]);
}

#[test]
fn irem_g101_has_two_prg_modes() {
    let mut cart = load(32, 16, 64);
    cart.cpu_write(0x8000, 3);
    cart.cpu_write(0xA000, 4);
    cart.cpu_write(0xB003, 20);
    assert_eq!(prg(&mut cart), [4, 5, 15, 16]);
    assert_eq!(chr(&cart)[3], 21);
    cart.cpu_write(0x9000, 0x03);
    assert_eq!(prg(&mut cart), [15, 5, 4, 16]);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
}

#[test]
fn jaleco_ss88006_nibble_registers_and_counter() {
    let mut cart = load(18, 32, 128);
    cart.cpu_write(0x8000, 0x03);
    cart.cpu_write(0x8001, 0x01);
    assert_eq!(prg(&mut cart)[0], 0x13 + 1);
    cart.cpu_write(0xA000, 0x05);
    cart.cpu_write(0xA001, 0x02);
    assert_eq!(chr(&cart)[0], 0x25 + 1);
    cart.cpu_write(0xF002, 1);
    assert_eq!(cart.mirroring, Mirroring::Vertical);

    // 4-bit counter: latch 2, enable, expect the IRQ on the third decrement.
    cart.cpu_write(0xE000, 0x02);
    cart.cpu_write(0xF000, 0);
    cart.cpu_write(0xF001, 0x09);
    cart.clock_cpu();
    cart.clock_cpu();
    assert!(!cart.irq_pending());
    cart.clock_cpu();
    cart.clock_cpu();
    assert!(cart.irq_pending());
    cart.cpu_write(0xF001, 0x00);
    assert!(!cart.irq_pending());
}

#[test]
fn taito_tc0190_with_irq_counts_scanlines() {
    let mut cart = load(48, 16, 64);
    cart.cpu_write(0x8000, 2);
    cart.cpu_write(0x8001, 3);
    cart.cpu_write(0xE000, 0x40);
    assert_eq!(prg(&mut cart), [3, 4, 15, 16]);
    assert_eq!(cart.mirroring, Mirroring::Horizontal);
    // Latch value 0xFF ^ 0xFD = 2: three clocks after the reload reach zero.
    cart.cpu_write(0xC000, 0xFD);
    cart.cpu_write(0xC001, 0);
    cart.cpu_write(0xC002, 0);
    cart.on_scanline();
    cart.on_scanline();
    assert!(!cart.irq_pending());
    cart.on_scanline();
    assert!(cart.irq_pending());
    cart.cpu_write(0xC003, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn irem_h3001_counts_down_cycles() {
    let mut cart = load(65, 16, 64);
    cart.cpu_write(0x8000, 2);
    cart.cpu_write(0xA000, 3);
    cart.cpu_write(0xC000, 4);
    cart.cpu_write(0xB005, 30);
    assert_eq!(prg(&mut cart), [3, 4, 5, 16]);
    assert_eq!(chr(&cart)[5], 31);
    cart.cpu_write(0x9005, 0);
    cart.cpu_write(0x9006, 3);
    cart.cpu_write(0x9004, 0);
    cart.cpu_write(0x9003, 0x80);
    cart.clock_cpu();
    cart.clock_cpu();
    assert!(!cart.irq_pending());
    cart.clock_cpu();
    assert!(cart.irq_pending());
}

#[test]
fn sunsoft3_two_kb_chr_prg_and_irq() {
    let mut cart = load(67, 16, 64);
    cart.cpu_write(0x8800, 3);
    cart.cpu_write(0xF800, 2);
    assert_eq!(&chr(&cart)[0..2], &[7, 8]);
    assert_eq!(prg(&mut cart), [5, 6, 15, 16]);
    // Counter 0x0002: high byte then low byte.
    cart.cpu_write(0xC800, 0);
    cart.cpu_write(0xC800, 2);
    cart.cpu_write(0xD800, 0x10);
    for _ in 0..2 {
        cart.clock_cpu();
        assert!(!cart.irq_pending());
    }
    cart.clock_cpu();
    cart.clock_cpu();
    assert!(cart.irq_pending());
    // The IRQ disables itself after firing and acknowledges on a control write.
    cart.cpu_write(0xD800, 0x00);
    assert!(!cart.irq_pending());
}

#[test]
fn vrc3_latch_irq_and_prg() {
    let mut cart = load(73, 16, 8);
    cart.cpu_write(0xF000, 3);
    assert_eq!(prg(&mut cart), [7, 8, 15, 16]);
    // 16-bit latch 0xFFFE: two clocks to overflow.
    cart.cpu_write(0x8000, 0x0E);
    cart.cpu_write(0x9000, 0x0F);
    cart.cpu_write(0xA000, 0x0F);
    cart.cpu_write(0xB000, 0x0F);
    cart.cpu_write(0xC000, 0x02);
    cart.clock_cpu();
    assert!(!cart.irq_pending());
    cart.clock_cpu();
    assert!(cart.irq_pending());
    cart.cpu_write(0xD000, 0);
    assert!(!cart.irq_pending());
}

#[test]
fn namco109_uses_four_two_kb_chr_banks() {
    let mut cart = load(76, 16, 64);
    let write = |cart: &mut Cartridge, reg: u8, value: u8| {
        cart.cpu_write(0x8000, reg);
        cart.cpu_write(0x8001, value);
    };
    write(&mut cart, 2, 1);
    write(&mut cart, 3, 2);
    write(&mut cart, 4, 3);
    write(&mut cart, 5, 4);
    write(&mut cart, 6, 2);
    write(&mut cart, 7, 3);
    assert_eq!(chr(&cart), [3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(prg(&mut cart), [3, 4, 15, 16]);
}

#[test]
fn asder_and_namco175_registers() {
    let mut cart = load(112, 16, 64);
    let write = |cart: &mut Cartridge, cmd: u8, value: u8| {
        cart.cpu_write(0x8000, cmd);
        cart.cpu_write(0xA000, value);
    };
    write(&mut cart, 0, 3);
    write(&mut cart, 1, 4);
    write(&mut cart, 2, 6);
    write(&mut cart, 5, 40);
    assert_eq!(prg(&mut cart), [4, 5, 15, 16]);
    assert_eq!(&chr(&cart)[0..2], &[7, 8]);
    assert_eq!(chr(&cart)[5], 41);

    let mut cart = load(210, 16, 64);
    cart.cpu_write(0x8800, 9);
    cart.cpu_write(0xE000, 0x82);
    cart.cpu_write(0xE800, 3);
    cart.cpu_write(0xF000, 4);
    assert_eq!(prg(&mut cart), [3, 4, 5, 16]);
    assert_eq!(chr(&cart)[1], 10);
    assert_eq!(cart.mirroring, Mirroring::OneScreenUpper);
}

#[test]
fn txsrom_takes_nametable_pages_from_chr_bank_bit_7() {
    let mut cart = load(118, 16, 128);
    let write = |cart: &mut Cartridge, reg: u8, value: u8| {
        cart.cpu_write(0x8000, reg);
        cart.cpu_write(0x8001, value);
    };
    write(&mut cart, 0, 0x80 | 4);
    write(&mut cart, 1, 8);
    // R0 (bit 7 set) covers $2000/$2400, R1 (bit 7 clear) covers $2800/$2C00.
    assert_eq!(cart.nametable_index(0x2000), 0x400);
    assert_eq!(cart.nametable_index(0x2400), 0x400);
    assert_eq!(cart.nametable_index(0x2800), 0);
    assert_eq!(cart.nametable_index(0x2C10), 0x10);
    // The CHR bank itself still ignores bit 7.
    assert_eq!(&chr(&cart)[0..2], &[5, 6]);
}

#[test]
fn namco3425_selects_nametable_pairs_from_r0_and_r1() {
    let mut cart = load(95, 16, 32);
    let write = |cart: &mut Cartridge, reg: u8, value: u8| {
        cart.cpu_write(0x8000, reg);
        cart.cpu_write(0x8001, value);
    };
    write(&mut cart, 0, 0x20 | 2);
    write(&mut cart, 1, 4);
    assert_eq!(cart.nametable_index(0x2000), 0x400);
    assert_eq!(cart.nametable_index(0x2400), 0x400);
    assert_eq!(cart.nametable_index(0x2800), 0);
    assert_eq!(&chr(&cart)[0..2], &[3, 4]);
}

/// Keeps the "73 mappers" figure in the README and UI honest.
#[test]
fn supported_mapper_count_matches_the_documentation() {
    let supported = (0u16..=255)
        .filter(|&mapper| {
            Cartridge::from_bytes(&ines(&prg_marked(16), &chr_marked(32), mapper, 0)).is_ok()
        })
        .count();
    assert_eq!(supported, 73);
}
