//! Full-system execution-level SoftMMU tests.
//!
//! These tests exercise the real JIT runtime path:
//! FullSystemCpu → gen_code → translate → regalloc →
//! codegen → cpu_exec_loop_env → helper dispatch →
//! fault delivery.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use machina_accel::exec::exec_loop::{cpu_exec_loop_env, ExitReason};
use machina_accel::exec::ExecEnv;
use machina_accel::x86_64::emitter::SoftMmuConfig;
use machina_accel::X86_64CodeGen;
use machina_core::address::GPA;
use machina_core::wfi::WfiWaker;
use machina_guest_riscv::riscv::cpu::RiscvCpu;
use machina_memory::address_space::AddressSpace;
use machina_memory::region::MemoryRegion;
use machina_system::cpus::{
    fault_cause_offset, fault_pc_offset, machina_mem_read, machina_mem_write,
    tlb_offsets, tlb_ptr_offset, FullSystemCpu, SharedMip, RAM_BASE, TLB_SIZE,
};

/// Build a SoftMmuConfig for test ExecEnv.
fn test_mmu_config() -> SoftMmuConfig {
    SoftMmuConfig {
        tlb_ptr_offset: tlb_ptr_offset(),
        entry_size: tlb_offsets::ENTRY_SIZE,
        addr_read_off: tlb_offsets::ADDR_READ,
        addr_write_off: tlb_offsets::ADDR_WRITE,
        addend_off: tlb_offsets::ADDEND,
        index_mask: (TLB_SIZE - 1) as u64,
        load_helper: machina_mem_read as *const () as u64,
        store_helper: machina_mem_write as *const () as u64,
        fault_cause_offset: fault_cause_offset(),
        fault_pc_offset: fault_pc_offset(),
        dirty_offset: tlb_offsets::DIRTY,
        tb_ret_addr: 0,
    }
}

/// Create a full-system test environment.
/// Returns (ExecEnv, FullSystemCpu, AddressSpace, ram_ptr).
/// `code` is written at RAM_BASE.
fn setup_fullsys(
    ram_size: u64,
    code: &[u8],
) -> (
    ExecEnv<X86_64CodeGen>,
    FullSystemCpu,
    Box<AddressSpace>,
    *const u8,
) {
    let mut backend = X86_64CodeGen::new();
    backend.mmio = Some(test_mmu_config());
    let env = ExecEnv::new(backend);

    // Create RAM-backed address space.
    let root = MemoryRegion::container("root", u64::MAX);
    let (ram_region, ram_block) = MemoryRegion::ram("ram", ram_size);
    let mut addr_space = Box::new(AddressSpace::new(root));
    addr_space
        .root_mut()
        .add_subregion(ram_region, GPA::new(RAM_BASE));
    addr_space.update_flat_view();

    let ram_ptr = ram_block.as_ptr() as *const u8;

    // Write test code at RAM_BASE.
    unsafe {
        std::ptr::copy_nonoverlapping(
            code.as_ptr(),
            ram_block.as_ptr(),
            code.len(),
        );
    }

    let shared_mip: SharedMip = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let wfi_waker = Arc::new(WfiWaker::new());
    let stop_flag = Arc::new(AtomicBool::new(true));

    let cpu = RiscvCpu::new();
    let fscpu = unsafe {
        FullSystemCpu::new(
            cpu,
            ram_ptr,
            ram_size,
            shared_mip,
            wfi_waker,
            &*addr_space as *const AddressSpace,
            stop_flag,
        )
    };

    // Set initial PC to RAM_BASE.
    // (FullSystemCpu::new sets guest_base, as_ptr, ram_end)

    (env, fscpu, addr_space, ram_ptr)
}

// RISC-V instruction encoders for test code.

/// ADDI rd, rs1, imm12 (I-type)
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm12 = (imm as u32) & 0xFFF;
    (imm12 << 20) | (rs1 << 15) | (0b000 << 12) | (rd << 7) | 0x13
}

/// LUI rd, imm20 (U-type)
fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}

/// ECALL
fn ecall() -> u32 {
    0x00000073
}

/// SD rs2, offset(rs1)
fn sd(rs2: u32, rs1: u32, offset: i32) -> u32 {
    let imm = (offset as u32) & 0xFFF;
    let imm_hi = (imm >> 5) & 0x7F;
    let imm_lo = imm & 0x1F;
    (imm_hi << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (0b011 << 12)
        | (imm_lo << 7)
        | 0x23
}

/// LD rd, offset(rs1)
fn ld(rd: u32, rs1: u32, offset: i32) -> u32 {
    let imm12 = (offset as u32) & 0xFFF;
    (imm12 << 20) | (rs1 << 15) | (0b011 << 12) | (rd << 7) | 0x03
}

/// Encode instructions to bytes (little-endian).
fn encode(insns: &[u32]) -> Vec<u8> {
    insns.iter().flat_map(|i| i.to_le_bytes()).collect()
}

// ═══════════════════════════════════════════════════════
// Full-system execution tests
// ═══════════════════════════════════════════════════════

/// Test: basic RISC-V code execution through the full
/// JIT pipeline. Loads a constant into x1 and ecalls.
#[test]
fn test_fullsys_basic_exec() {
    let code = encode(&[
        addi(1, 0, 42), // x1 = 42
        addi(2, 0, 99), // x2 = 99
        ecall(),        // exit
    ]);

    let (mut env, mut cpu, _as, _ram) = setup_fullsys(1024 * 1024, &code);
    cpu.cpu.pc = RAM_BASE;

    let r = unsafe { cpu_exec_loop_env(&mut env, &mut cpu) };

    assert_eq!(r, ExitReason::Ecall { priv_level: 3 },);
    assert_eq!(cpu.cpu.gpr[1], 42);
    assert_eq!(cpu.cpu.gpr[2], 99);
}

/// Test: RAM load/store through TLB in M-mode BARE.
/// Stores a value then loads it back.
#[test]
fn test_fullsys_ram_load_store() {
    // Use AUIPC to get a PC-relative address in RAM.
    // AUIPC rd, imm20: rd = PC + (imm20 << 12)
    // At PC=0x80000000, auipc x3, 0 → x3 = 0x80000000
    // Then addi x3, x3, 0x100 → x3 = 0x80000100
    fn auipc(rd: u32, imm20: u32) -> u32 {
        (imm20 << 12) | (rd << 7) | 0x17
    }
    let code = encode(&[
        auipc(3, 0),       // x3 = PC = 0x80000000
        addi(3, 3, 0x100), // x3 += 0x100
        addi(1, 0, 0x55),  // x1 = 0x55
        sd(1, 3, 0),       // *(x3) = x1
        ld(2, 3, 0),       // x2 = *(x3)
        ecall(),
    ]);

    let (mut env, mut cpu, _as, _ram) = setup_fullsys(1024 * 1024, &code);
    cpu.cpu.pc = RAM_BASE;

    let r = unsafe { cpu_exec_loop_env(&mut env, &mut cpu) };

    assert_eq!(r, ExitReason::Ecall { priv_level: 3 },);
    // x2 should have the stored value.
    assert_eq!(cpu.cpu.gpr[2], 0x55);
}

/// Test: MMIO write goes through AddressSpace (not
/// fast-path RAM). Write to unmapped MMIO address
/// produces a fault.
#[test]
fn test_fullsys_mmio_write_no_crash() {
    // Write to address 0x1000_0000 (UART range).
    // No UART device mapped → AddressSpace silently
    // drops the write (unmapped write returns).
    let code = encode(&[
        lui(3, 0x10000),  // x3 = 0x10000000
        addi(1, 0, 0x41), // x1 = 'A'
        sd(1, 3, 0),      // *(0x10000000) = 'A'
        ecall(),
    ]);

    let (mut env, mut cpu, _as, _ram) = setup_fullsys(1024 * 1024, &code);
    cpu.cpu.pc = RAM_BASE;

    let r = unsafe { cpu_exec_loop_env(&mut env, &mut cpu) };

    // Should reach ecall without crash.
    assert_eq!(r, ExitReason::Ecall { priv_level: 3 },);
}
