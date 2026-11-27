use nes_core::apu::{Apu, CPU_CLOCK_HZ};

const RATE: u32 = 44_100;

/// The triangle timer runs at the CPU rate, so period `p` gives
/// `CPU_CLOCK_HZ / (32 * (p + 1))` Hz. It used to run at half that rate.
#[test]
fn triangle_plays_at_the_right_pitch() {
    let mut apu = Apu::new(RATE);
    apu.write_register(0x4015, 0x04);
    apu.write_register(0x4008, 0xFF);
    apu.write_register(0x400A, 253);
    apu.write_register(0x400B, 0x00);
    // Let the linear counter load, then count full wave cycles for 0.1 s.
    for _ in 0..20_000 {
        apu.step();
    }
    let cycles = (CPU_CLOCK_HZ * 0.1) as usize;
    let mut wraps = 0;
    let mut previous = apu.state.triangle.step;
    for _ in 0..cycles {
        apu.step();
        let step = apu.state.triangle.step;
        if step < previous {
            wraps += 1;
        }
        previous = step;
    }
    let expected = (CPU_CLOCK_HZ / (32.0 * 254.0) * 0.1).round() as i32;
    assert!(
        (wraps - expected).abs() <= 1,
        "expected about {expected} wave cycles, got {wraps}"
    );
}

/// Noise period index 0 shifts the LFSR every 4 CPU cycles.
#[test]
fn noise_shifts_at_the_cpu_rate() {
    let mut apu = Apu::new(RATE);
    apu.write_register(0x400E, 0x00);
    let mut shifts = 0;
    let mut previous = apu.state.noise.shift;
    for _ in 0..4_000 {
        apu.step();
        if apu.state.noise.shift != previous {
            shifts += 1;
            previous = apu.state.noise.shift;
        }
    }
    assert!((shifts - 1_000i32).abs() <= 2, "got {shifts} shifts");
}

/// Silence must stay silent (no DC offset), and a loud tone must stay inside
/// the legal sample range with no clipping spikes.
#[test]
fn output_has_no_dc_offset_and_stays_in_range() {
    let mut apu = Apu::new(RATE);
    let mut out = Vec::new();
    for _ in 0..CPU_CLOCK_HZ as usize / 2 {
        apu.step();
    }
    apu.drain_samples(&mut out);
    assert!(out.iter().skip(2_000).all(|s| s.abs() < 0.001));

    apu.write_register(0x4015, 0x01);
    apu.write_register(0x4000, 0xBF);
    apu.write_register(0x4002, 0xFD);
    apu.write_register(0x4003, 0x00);
    for _ in 0..CPU_CLOCK_HZ as usize / 4 {
        apu.step();
    }
    out.clear();
    apu.drain_samples(&mut out);
    assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
    assert!(out.iter().any(|s| s.abs() > 0.05), "tone should be audible");
}
