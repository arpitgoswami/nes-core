use nes_core::cpu::{
    Cpu, CpuBus, FLAG_CARRY, FLAG_IRQ_DISABLE, FLAG_NEGATIVE, FLAG_OVERFLOW, FLAG_ZERO,
};

struct TestBus {
    mem: Vec<u8>,
    irq: bool,
    nmi: bool,
    stalls: u32,
}

impl CpuBus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[usize::from(addr)]
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.mem[usize::from(addr)] = value;
    }

    fn poll_nmi(&mut self) -> bool {
        std::mem::take(&mut self.nmi)
    }

    fn poll_irq(&self) -> bool {
        self.irq
    }

    fn take_stall_cycles(&mut self) -> u32 {
        std::mem::take(&mut self.stalls)
    }
}

fn setup(program: &[u8]) -> (Cpu, TestBus) {
    let mut mem = vec![0u8; 0x1_0000];
    mem[0x8000..0x8000 + program.len()].copy_from_slice(program);
    mem[0xFFFC] = 0x00;
    mem[0xFFFD] = 0x80;
    mem[0xFFFA] = 0x00;
    mem[0xFFFB] = 0x90;
    mem[0xFFFE] = 0x00;
    mem[0xFFFF] = 0xA0;
    let mut bus = TestBus {
        mem,
        irq: false,
        nmi: false,
        stalls: 0,
    };
    let mut cpu = Cpu::new();
    cpu.reset(&mut bus);
    (cpu, bus)
}

#[test]
fn reset_reads_vector_and_initializes_registers() {
    let (cpu, _) = setup(&[]);
    assert_eq!(cpu.pc, 0x8000);
    assert_eq!(cpu.sp, 0xFA);
    assert!(cpu.flag(FLAG_IRQ_DISABLE));
}

#[test]
fn load_and_store_with_zero_page() {
    // LDX #$05; LDA #$42; STA $00; LDY $00; INX
    let (mut cpu, mut bus) = setup(&[0xA2, 0x05, 0xA9, 0x42, 0x85, 0x00, 0xA4, 0x00, 0xE8]);
    cpu.step(&mut bus);
    assert_eq!(cpu.x, 5);
    let cycles = cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 2);
    cpu.step(&mut bus);
    assert_eq!(bus.mem[0x00], 0x42);
    cpu.step(&mut bus);
    assert_eq!(cpu.y, 0x42);
    assert!(!cpu.flag(FLAG_ZERO));
    cpu.step(&mut bus);
    assert_eq!(cpu.x, 6);
}

#[test]
fn adc_sets_carry_overflow_and_negative() {
    // CLC; LDA #$7F; ADC #$01
    let (mut cpu, mut bus) = setup(&[0x18, 0xA9, 0x7F, 0x69, 0x01]);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag(FLAG_OVERFLOW));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_CARRY));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn sbc_handles_borrow() {
    // SEC; LDA #$00; SBC #$01
    let (mut cpu, mut bus) = setup(&[0x38, 0xA9, 0x00, 0xE9, 0x01]);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0xFF);
    assert!(!cpu.flag(FLAG_CARRY));
    assert!(cpu.flag(FLAG_NEGATIVE));
}

#[test]
fn taken_branch_and_page_cross_cycles() {
    // CLC; BCC +2 (taken, +1)
    let (mut cpu, mut bus) = setup(&[0x18, 0x90, 0x02, 0xEA, 0xEA, 0xEA]);
    cpu.step(&mut bus);
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 3);
    assert_eq!(cpu.pc, 0x8005);
}

#[test]
fn jsr_and_rts_restore_stack_pointer() {
    // JSR $8006; ... ; RTS  (the subroutine is a NOP and RTS)
    let (mut cpu, mut bus) = setup(&[0x20, 0x06, 0x80, 0xEA, 0xEA, 0xEA, 0x60]);
    let before = cpu.sp;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8006);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8003);
    assert_eq!(cpu.sp, before);
}

#[test]
fn indirect_jmp_reproduces_page_wrap_bug() {
    let (mut cpu, mut bus) = setup(&[0x6C, 0xFF, 0x02]);
    bus.mem[0x02FF] = 0x34;
    bus.mem[0x0200] = 0x12;
    bus.mem[0x0300] = 0x99;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn indexed_reads_add_a_cycle_on_page_cross() {
    // LDX #$01; LDA $80FF,X
    let (mut cpu, mut bus) = setup(&[0xA2, 0x01, 0xBD, 0xFF, 0x80]);
    bus.mem[0x8100] = 0x77;
    cpu.step(&mut bus);
    let cycles = cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x77);
    assert_eq!(cycles, 5);
}

#[test]
fn nmi_uses_nmi_vector() {
    let (mut cpu, mut bus) = setup(&[0xEA]);
    bus.nmi = true;
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(cpu.pc, 0x9000);
    let sp = cpu.sp;
    assert_eq!(bus.mem[0x1FA], 0x80); // return address high
    assert_eq!(bus.mem[0x1F9], 0x00);
    assert_eq!(sp, 0xFA - 3);
}

#[test]
fn masked_irq_is_ignored_until_cli() {
    let (mut cpu, mut bus) = setup(&[0xEA]);
    bus.irq = true;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8001);
    // CLI then NOP: the IRQ is taken at the next instruction boundary.
    bus.mem[0x8000] = 0x58;
    bus.mem[0x8001] = 0xEA;
    cpu.pc = 0x8000;
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0xA000);
}

#[test]
fn unofficial_dcp_decrements_and_compares() {
    // LDA #$05; DCP $10
    let (mut cpu, mut bus) = setup(&[0xA9, 0x05, 0xC7, 0x10]);
    bus.mem[0x10] = 0x05;
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(bus.mem[0x10], 0x04);
    assert!(cpu.flag(FLAG_CARRY));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn dma_stall_is_added_to_instruction_cycles() {
    let (mut cpu, mut bus) = setup(&[0xEA, 0xEA]);
    bus.stalls = 513;
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 2 + 513);
}
