use std::cell::RefCell;
use std::rc::Rc;

use nes_core::cartridge::Cartridge;
use nes_core::ppu::palette::PALETTE_RGB;
use nes_core::ppu::Ppu;

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

/// An MMC5 image whose 4 KB CHR banks hold one distinguishable tile each:
/// bank 0 uses colour 1, bank 1 colour 2 and bank 2 (tile 1) colour 3.
fn mmc5_ppu() -> (Ppu, Rc<RefCell<Cartridge>>) {
    let prg = vec![0u8; 65536];
    let mut chr = vec![0u8; 32768];
    chr[0x00..0x08].fill(0xFF);
    chr[0x1008..0x1010].fill(0xFF);
    chr[0x2000 + 16..0x2000 + 32].fill(0xFF);

    let cart = Cartridge::from_bytes(&ines(&prg, &chr, 5, 0)).expect("synthetic MMC5 ROM");
    let cart = Rc::new(RefCell::new(cart));
    let mut ppu = Ppu::new(cart.clone());

    ppu.state.palette[0] = 0x0F;
    ppu.state.palette[1] = 0x21;
    ppu.state.palette[3] = 0x24;
    ppu.state.palette[6] = 0x31;
    for byte in ppu.state.vram[..0x400].iter_mut() {
        *byte = 0;
    }
    // Background plus left-side background, no sprites.
    ppu.state.mask = 0x0A;
    (ppu, cart)
}

fn render_line_zero(ppu: &mut Ppu) {
    ppu.state.scanline = 0;
    ppu.state.dot = 0;
    ppu.state.v = 0;
    ppu.step();
}

fn pixel(ppu: &Ppu, x: usize) -> [u8; 4] {
    let o = x * 4;
    [
        ppu.framebuffer[o],
        ppu.framebuffer[o + 1],
        ppu.framebuffer[o + 2],
        ppu.framebuffer[o + 3],
    ]
}

fn rgba(color: (u8, u8, u8)) -> [u8; 4] {
    [color.0, color.1, color.2, 255]
}

#[test]
fn mmc5_extended_attributes_change_palette_and_chr_bank() {
    let (mut ppu, cart) = mmc5_ppu();
    render_line_zero(&mut ppu);
    assert_eq!(pixel(&ppu, 0), rgba(PALETTE_RGB[0x21]));

    {
        let mut cart = cart.borrow_mut();
        cart.cpu_write(0x5104, 1);
        // Palette 1, 4 KB CHR bank 1: bank 1 tile 0 draws colour 2.
        cart.cpu_write(0x5C00, 0x41);
    }
    render_line_zero(&mut ppu);
    assert_eq!(pixel(&ppu, 0), rgba(PALETTE_RGB[0x31]));
}

#[test]
fn mmc5_vertical_split_draws_exram_tiles() {
    let (mut ppu, cart) = mmc5_ppu();
    {
        let mut cart = cart.borrow_mut();
        cart.cpu_write(0x5104, 0);
        cart.cpu_write(0x5200, 0x80 | 16); // enabled, left side, 16 tile threshold
        cart.cpu_write(0x5201, 0x00);
        cart.cpu_write(0x5202, 2); // split CHR is 4 KB bank 2
        cart.cpu_write(0x5C00, 1); // column 0 shows tile 1 from bank 2
    }
    render_line_zero(&mut ppu);

    // Left region: split tile 1 in bank 2 with palette 0 gives colour 3.
    assert_eq!(pixel(&ppu, 0), rgba(PALETTE_RGB[0x24]));
    assert_eq!(pixel(&ppu, 7), rgba(PALETTE_RGB[0x24]));
    // Column 15 is an empty split tile, so the backdrop shows through.
    assert_eq!(pixel(&ppu, 127), rgba(PALETTE_RGB[0x0F]));
    // Right region keeps rendering from the normal nametable.
    assert_eq!(pixel(&ppu, 128), rgba(PALETTE_RGB[0x21]));
    assert_eq!(pixel(&ppu, 255), rgba(PALETTE_RGB[0x21]));
}

/// $2005 scroll must reach the renderer through the t -> v copies.
#[test]
fn scroll_written_through_2005_moves_the_background() {
    let prg = vec![0u8; 16384];
    let mut chr = vec![0u8; 8192];
    chr[16..24].fill(0xFF); // tile 1: solid colour 1
    let cart = Rc::new(RefCell::new(
        Cartridge::from_bytes(&ines(&prg, &chr, 0, 1)).unwrap(),
    ));
    let mut ppu = Ppu::new(cart);
    // Tile 1 at nametable 0, row 0, column 1.
    ppu.write_register(6, 0x20);
    ppu.write_register(6, 0x01);
    ppu.write_register(7, 1);
    // Palette 0, colour 1.
    ppu.write_register(6, 0x3F);
    ppu.write_register(6, 0x01);
    ppu.write_register(7, 0x16);
    ppu.write_register(0, 0); // like real games: reselect nametable 0 after VRAM setup
    // Scroll X by 8 pixels, nothing else, then enable the background.
    ppu.write_register(5, 8);
    ppu.write_register(5, 0);
    ppu.write_register(1, 0x0A);
    for _ in 0..(262 * 341 * 2) {
        ppu.step();
    }
    let expected = PALETTE_RGB[0x16];
    let px = &ppu.framebuffer[0..4];
    assert_eq!(
        (px[0], px[1], px[2]),
        expected,
        "tile at column 1 should be scrolled to x = 0"
    );
}
