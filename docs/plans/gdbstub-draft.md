# Machina GDBStub Enhancement Draft

## Goal

Enhance the machina gdbstub implementation to align with QEMU's gdbstub functionality,
providing a complete GDB remote debugging interface for the RISC-V emulator.

## Current State

Machina has initial gdbstub work in progress (gdbstub/ directory and system/src/gdb.rs,
tests/src/gdbstub.rs are newly added). The goal is to bring this to feature parity with
QEMU's gdbstub implementation.

## Target Features (Aligned with QEMU gdbstub)

1. GDB Remote Serial Protocol (RSP) server over TCP
2. Register read/write (general purpose, FP, CSR registers)
3. Memory read/write operations
4. Software breakpoints (sw break via EBREAK insertion)
5. Hardware breakpoints and watchpoints
6. Single-step execution
7. vCPU start/stop/continue control
8. Multi-threaded debugging (multi-vCPU support)
9. XML target description (register layout advertisement)
10. Signal handling and stop-reply packets
11. Integration with the existing monitor console
12. Kill/detach handling

## QEMU Reference

The primary QEMU reference files are:
- ~/qemu/gdbstub/gdbstub.c - Core gdbstub protocol handling
- ~/qemu/gdbstub/syscalls.c - Syscall handling
- ~/qemu/gdbstub/user.c - User-mode specific
- ~/qemu/gdbstub/system.c - System-mode specific
- ~/qemu/include/exec/gdbstub.h - Public API
- ~/qemu/target/riscv/gdbstub.c - RISC-V specific register handling
- ~/qemu/configs/targets/riscv64-softmmu.mak - Target config
