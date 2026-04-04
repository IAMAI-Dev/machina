// GDB stub state management.
//
// Coordinates between the GDB server thread and the
// CPU execution loop for breakpoints, single-step,
// and pause/resume. Includes register snapshot for
// cross-thread CPU state access.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// CPU run state from the GDB stub's perspective.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GdbRunState {
    Running,
    PauseRequested,
    Paused,
    Stepping,
}

/// Snapshot of RISC-V CPU registers for GDB access.
/// Filled by the exec loop when CPU pauses, read by
/// the GDB server thread.
#[derive(Clone)]
pub struct GdbCpuSnapshot {
    /// x0-x31 general-purpose registers.
    pub gpr: [u64; 32],
    /// f0-f31 floating-point registers.
    pub fpr: [u64; 32],
    /// Program counter.
    pub pc: u64,
    /// Set when GDB writes registers that need to be
    /// restored before the CPU resumes.
    pub dirty: bool,
}

impl Default for GdbCpuSnapshot {
    fn default() -> Self {
        Self {
            gpr: [0u64; 32],
            fpr: [0u64; 32],
            pc: 0,
            dirty: false,
        }
    }
}

/// Shared GDB debug state between the server and exec loop.
pub struct GdbState {
    inner: Mutex<GdbInner>,
    /// Condvar signaled when exec loop parks.
    pause_cv: Condvar,
    /// Condvar signaled when GDB resumes.
    resume_cv: Condvar,
    /// Whether a GDB client is connected.
    connected: AtomicBool,
    /// CPU register snapshot (valid only when paused).
    snapshot: Mutex<GdbCpuSnapshot>,
    /// Host pointer to guest RAM for memory access.
    ram_ptr: AtomicU64,
    /// Guest RAM size in bytes.
    ram_size: AtomicU64,
    /// Guest RAM end (base + size).
    ram_end: AtomicU64,
    /// Host pointer to AddressSpace for MMIO.
    as_ptr: AtomicU64,
}

struct GdbInner {
    state: GdbRunState,
    stop_reason: StopReason,
    breakpoints: BTreeSet<u64>,
    hw_breakpoints: BTreeSet<u64>,
    detached: bool,
}

impl GdbState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(GdbInner {
                state: GdbRunState::Paused,
                stop_reason: StopReason::Pause,
                breakpoints: BTreeSet::new(),
                hw_breakpoints: BTreeSet::new(),
                detached: false,
            }),
            pause_cv: Condvar::new(),
            resume_cv: Condvar::new(),
            connected: AtomicBool::new(false),
            snapshot: Mutex::new(GdbCpuSnapshot::default()),
            ram_ptr: AtomicU64::new(0),
            ram_size: AtomicU64::new(0),
            ram_end: AtomicU64::new(0),
            as_ptr: AtomicU64::new(0),
        }
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::SeqCst);
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    // -- Memory access configuration --

    /// Set the host RAM pointer and size for memory R/W.
    /// Called once during machine setup.
    pub fn set_mem_access(
        &self,
        ram_ptr: *const u8,
        ram_size: u64,
        ram_base: u64,
        as_ptr: u64,
    ) {
        self.ram_ptr.store(
            ram_ptr as u64,
            Ordering::SeqCst,
        );
        self.ram_size.store(ram_size, Ordering::SeqCst);
        self.ram_end.store(
            ram_base + ram_size,
            Ordering::SeqCst,
        );
        self.as_ptr.store(as_ptr, Ordering::SeqCst);
    }

    /// Read guest memory at physical address.
    pub fn read_memory(&self, addr: u64, len: usize) -> Vec<u8> {
        let ram_ptr = self.ram_ptr.load(Ordering::SeqCst);
        let ram_end = self.ram_end.load(Ordering::SeqCst);
        let as_ptr = self.as_ptr.load(Ordering::SeqCst);
        if ram_ptr == 0 || len == 0 {
            return vec![0; len];
        }
        let ram_base = 0x8000_0000u64; // RAM_BASE
        if addr >= ram_base
            && addr + len as u64 <= ram_end
        {
            let off = (addr - ram_base) as usize;
            let ptr = unsafe {
                (ram_ptr as *const u8).add(off)
            };
            let mut buf = vec![0u8; len];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    ptr,
                    buf.as_mut_ptr(),
                    len,
                );
            }
            buf
        } else if as_ptr != 0 {
            // MMIO: fall back to AddressSpace.
            let mut buf = vec![0u8; len];
            use machina_core::address::GPA;
            use machina_memory::address_space::AddressSpace;
            let as_ =
                unsafe { &*(as_ptr as *const AddressSpace) };
            for (i, byte) in buf.iter_mut().enumerate() {
                *byte = as_
                    .read(GPA::new(addr + i as u64), 1)
                    as u8;
            }
            buf
        } else {
            vec![0; len]
        }
    }

    /// Write guest memory at physical address.
    pub fn write_memory(
        &self,
        addr: u64,
        data: &[u8],
    ) -> bool {
        let ram_ptr = self.ram_ptr.load(Ordering::SeqCst);
        let ram_end = self.ram_end.load(Ordering::SeqCst);
        let as_ptr = self.as_ptr.load(Ordering::SeqCst);
        if ram_ptr == 0 || data.is_empty() {
            return false;
        }
        let ram_base = 0x8000_0000u64;
        if addr >= ram_base
            && addr + data.len() as u64 <= ram_end
        {
            let off = (addr - ram_base) as usize;
            let ptr = unsafe {
                (ram_ptr as *mut u8).add(off)
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    ptr,
                    data.len(),
                );
            }
            true
        } else if as_ptr != 0 {
            use machina_core::address::GPA;
            use machina_memory::address_space::AddressSpace;
            let as_ =
                unsafe { &*(as_ptr as *const AddressSpace) };
            for (i, &byte) in data.iter().enumerate() {
                as_.write(
                    GPA::new(addr + i as u64),
                    1,
                    byte as u64,
                );
            }
            true
        } else {
            false
        }
    }

    // -- Register snapshot --

    /// Save CPU register state into the snapshot.
    /// Called by the exec loop when the CPU pauses.
    pub fn save_snapshot(&self, gpr: &[u64; 32], fpr: &[u64; 32], pc: u64) {
        let mut snap = self.snapshot.lock().unwrap();
        snap.gpr.copy_from_slice(gpr);
        snap.fpr.copy_from_slice(fpr);
        snap.pc = pc;
        snap.dirty = false;
    }

    /// Get a clone of the current register snapshot.
    pub fn read_snapshot(&self) -> GdbCpuSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    /// Write back modified registers from the snapshot.
    /// Called by the exec loop before resuming.
    /// Returns the snapshot if dirty (registers need
    /// restoring), None otherwise.
    pub fn take_dirty_snapshot(&self) -> Option<GdbCpuSnapshot> {
        let mut snap = self.snapshot.lock().unwrap();
        if snap.dirty {
            snap.dirty = false;
            Some(snap.clone())
        } else {
            None
        }
    }

    /// Write a single register in the snapshot.
    /// reg: 0-31=GPR, 32=PC, 33-64=FPR.
    pub fn write_register(
        &self,
        reg: usize,
        val: u64,
    ) -> bool {
        let mut snap = self.snapshot.lock().unwrap();
        match reg {
            0 => { /* x0 hardwired to 0 */ }
            1..=31 => snap.gpr[reg] = val,
            32 => snap.pc = val,
            33..=64 => snap.fpr[reg - 33] = val,
            _ => return false,
        }
        snap.dirty = true;
        true
    }

    // -- Breakpoint management --

    pub fn set_breakpoint(&self, addr: u64) -> bool {
        self.inner.lock().unwrap().breakpoints.insert(addr);
        true
    }

    pub fn remove_breakpoint(&self, addr: u64) -> bool {
        self.inner.lock().unwrap().breakpoints.remove(&addr);
        true
    }

    pub fn set_hw_breakpoint(&self, addr: u64) -> bool {
        self.inner.lock().unwrap().hw_breakpoints.insert(addr);
        true
    }

    pub fn remove_hw_breakpoint(&self, addr: u64) -> bool {
        self.inner.lock().unwrap().hw_breakpoints.remove(&addr);
        true
    }

    pub fn hit_breakpoint(&self, pc: u64) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.breakpoints.contains(&pc)
            || inner.hw_breakpoints.contains(&pc)
    }

    pub fn has_breakpoints(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.breakpoints.is_empty()
            || !inner.hw_breakpoints.is_empty()
    }

    // -- Run state management --

    /// Set the stop reason (called from exec loop).
    pub fn set_stop_reason(&self, reason: StopReason) {
        self.inner.lock().unwrap().stop_reason = reason;
    }

    /// Get the current stop reason.
    pub fn get_stop_reason(&self) -> StopReason {
        self.inner.lock().unwrap().stop_reason
    }

    pub fn run_state(&self) -> GdbRunState {
        self.inner.lock().unwrap().state
    }

    /// Request the CPU to pause (non-blocking).
    pub fn request_pause(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.state == GdbRunState::Paused {
            return;
        }
        inner.state = GdbRunState::PauseRequested;
    }

    /// Wait until the exec loop has parked.
    pub fn wait_paused(&self) {
        let mut inner = self.inner.lock().unwrap();
        while inner.state != GdbRunState::Paused {
            inner = self.pause_cv.wait(inner).unwrap();
        }
    }

    /// Wait until the exec loop has parked, with timeout.
    /// Returns true if paused, false if timed out.
    pub fn wait_paused_timeout(&self, timeout: Duration) -> bool {
        let inner = self.inner.lock().unwrap();
        if inner.state == GdbRunState::Paused {
            return true;
        }
        let result =
            self.pause_cv.wait_timeout(inner, timeout).unwrap();
        result.0.state == GdbRunState::Paused
    }

    /// Resume the CPU from paused state.
    pub fn request_resume(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.state = GdbRunState::Running;
        self.resume_cv.notify_all();
    }

    /// Request single-step.
    pub fn request_step(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.state = GdbRunState::Stepping;
        self.resume_cv.notify_all();
    }

    pub fn detach(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.detached = true;
        inner.state = GdbRunState::Running;
        self.resume_cv.notify_all();
        self.connected.store(false, Ordering::SeqCst);
    }

    pub fn is_detached(&self) -> bool {
        self.inner.lock().unwrap().detached
    }

    /// Called by the exec loop to check if it should
    /// pause. If PauseRequested/Paused, parks and
    /// blocks until resumed. Returns true if quit.
    pub fn check_and_wait(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.detached {
            return false;
        }
        match inner.state {
            GdbRunState::PauseRequested
            | GdbRunState::Paused => {
                inner.state = GdbRunState::Paused;
                self.pause_cv.notify_all();
                while inner.state == GdbRunState::Paused {
                    inner =
                        self.resume_cv.wait(inner).unwrap();
                }
                false
            }
            _ => false,
        }
    }

    /// Called by the exec loop after executing one TB in
    /// stepping mode. Transitions Stepping -> Paused.
    pub fn complete_step(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.state == GdbRunState::Stepping {
            inner.state = GdbRunState::Paused;
            self.pause_cv.notify_all();
            while inner.state == GdbRunState::Paused {
                inner =
                    self.resume_cv.wait(inner).unwrap();
            }
            return true;
        }
        false
    }

    pub fn is_stepping(&self) -> bool {
        self.inner.lock().unwrap().state
            == GdbRunState::Stepping
    }
}

impl Default for GdbState {
    fn default() -> Self {
        Self::new()
    }
}

// ---- GdbStateTarget: GdbTarget via GdbState ----

use machina_gdbstub::handler::{
    GdbHandler, GdbTarget, StopReason,
};
use machina_gdbstub::protocol;

const NUM_GPRS: usize = 32;
const NUM_FPRS: usize = 32;
const GDB_NUM_REGS: usize = NUM_GPRS + 1 + NUM_FPRS;

/// Bridge that implements `GdbTarget` by delegating to
/// `GdbState` for cross-thread CPU access.
pub struct GdbStateTarget<'a> {
    gs: &'a GdbState,
}

impl<'a> GdbStateTarget<'a> {
    pub fn new(gs: &'a GdbState) -> Self {
        Self { gs }
    }
}

impl GdbTarget for GdbStateTarget<'_> {
    fn read_registers(&self) -> Vec<u8> {
        let snap = self.gs.read_snapshot();
        let mut buf = Vec::with_capacity(GDB_NUM_REGS * 8);
        for &val in &snap.gpr {
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf.extend_from_slice(&snap.pc.to_le_bytes());
        for &val in &snap.fpr {
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    fn write_registers(
        &mut self,
        _data: &[u8],
    ) -> bool {
        // Not supported through snapshot; would need
        // per-register dirty tracking.
        false
    }

    fn read_register(&self, reg: usize) -> Vec<u8> {
        let snap = self.gs.read_snapshot();
        match reg {
            0..=31 => snap.gpr[reg].to_le_bytes().to_vec(),
            32 => snap.pc.to_le_bytes().to_vec(),
            33..=64 => {
                snap.fpr[reg - 33].to_le_bytes().to_vec()
            }
            _ => Vec::new(),
        }
    }

    fn write_register(
        &mut self,
        reg: usize,
        val: &[u8],
    ) -> bool {
        if val.len() < 8 {
            return false;
        }
        let v = u64::from_le_bytes(
            val[..8].try_into().unwrap(),
        );
        self.gs.write_register(reg, v)
    }

    fn read_memory(
        &self,
        addr: u64,
        len: usize,
    ) -> Vec<u8> {
        self.gs.read_memory(addr, len)
    }

    fn write_memory(
        &mut self,
        addr: u64,
        data: &[u8],
    ) -> bool {
        self.gs.write_memory(addr, data)
    }

    fn set_breakpoint(
        &mut self,
        type_: u8,
        addr: u64,
        _kind: u32,
    ) -> bool {
        match type_ {
            0 => self.gs.set_breakpoint(addr),
            1 => self.gs.set_hw_breakpoint(addr),
            _ => false,
        }
    }

    fn remove_breakpoint(
        &mut self,
        type_: u8,
        addr: u64,
        _kind: u32,
    ) -> bool {
        match type_ {
            0 => self.gs.remove_breakpoint(addr),
            1 => self.gs.remove_hw_breakpoint(addr),
            _ => false,
        }
    }

    fn resume(&mut self) {
        // Non-blocking: just signal resume. The serve()
        // loop owns the wait-for-stop cycle.
        self.gs.request_resume();
    }

    fn step(&mut self) {
        // Non-blocking: just signal step. The serve()
        // loop owns the wait-for-stop cycle.
        self.gs.request_step();
    }

    fn get_pc(&self) -> u64 {
        self.gs.read_snapshot().pc
    }

    fn get_stop_reason(&self) -> StopReason {
        self.gs.get_stop_reason()
    }
}

// ---- Resume/step action detection ----

/// Resume action intercepted by serve().
enum ResumeAction {
    Continue,
    Step,
}

/// Check if a packet is a resume/step command that
/// serve() should handle directly (with Ctrl-C support)
/// instead of delegating to the handler.
fn check_resume_packet(packet: &str) -> Option<ResumeAction> {
    let first = packet.chars().next()?;
    match first {
        'c' => Some(ResumeAction::Continue),
        'C' => Some(ResumeAction::Continue),
        's' => Some(ResumeAction::Step),
        'S' => Some(ResumeAction::Step),
        'v' => {
            let rest = packet.strip_prefix("vCont;")?;
            if rest.is_empty() {
                return None;
            }
            let first_action =
                rest.split(';').next().unwrap_or("");
            let cmd =
                first_action.split(':').next().unwrap_or("");
            match cmd {
                "c" | "C" => Some(ResumeAction::Continue),
                "s" | "S" => Some(ResumeAction::Step),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Format a GDB stop reply for the given reason.
fn stop_reply(reason: StopReason) -> String {
    match reason {
        StopReason::Breakpoint => {
            "T05thread:01;swbreak:;".to_string()
        }
        StopReason::Step => "S05".to_string(),
        StopReason::Pause => "T02thread:01;".to_string(),
        StopReason::Terminated => "W00".to_string(),
    }
}

/// Wait for the CPU to stop during a continue, while
/// polling the TCP socket for Ctrl-C (0x03 byte).
/// Uses non-blocking peek to avoid consuming data.
fn wait_for_stop_with_ctrl_c(
    gs: &GdbState,
    stream: &mut std::net::TcpStream,
) -> std::io::Result<StopReason> {
    stream.set_nonblocking(true)?;
    let result = loop {
        // Check if CPU has paused.
        if gs.wait_paused_timeout(Duration::from_millis(
            50,
        )) {
            break Ok(gs.get_stop_reason());
        }

        // Timeout: check for Ctrl-C on socket.
        let mut peek_buf = [0u8; 1];
        match stream.peek(&mut peek_buf) {
            Ok(1) if peek_buf[0] == 0x03 => {
                // Consume the Ctrl-C byte.
                use std::io::Read;
                let _ = stream.read(&mut peek_buf);
                gs.set_stop_reason(StopReason::Pause);
                gs.request_pause();
                // Continue loop to wait for CPU to
                // actually park.
            }
            Ok(_) => {
                // Unexpected data during continue.
                // Break and let main loop handle it.
                break Ok(gs.get_stop_reason());
            }
            Err(ref e)
                if e.kind()
                    == std::io::ErrorKind::WouldBlock =>
            {
                // No Ctrl-C, loop back.
            }
            Err(e) => break Err(e),
        }
    };
    stream.set_nonblocking(false)?;
    result
}

// ---- GDB server entry point ----

/// Run the GDB RSP server loop on an accepted TCP stream.
///
/// Handles c/s/vCont by resuming the CPU and waiting for
/// a stop event (breakpoint, step completion, Ctrl-C) with
/// non-blocking Ctrl-C polling. All other packets are
/// dispatched to GdbHandler.
pub fn serve(
    mut stream: std::net::TcpStream,
    gs: &GdbState,
) -> std::io::Result<()> {
    stream.set_nodelay(true)?;

    // Wait for CPU to be paused.
    gs.request_pause();
    gs.wait_paused();

    let mut target = GdbStateTarget::new(gs);
    let mut handler = GdbHandler::new();
    // Initial stop reply: SIGTRAP on attach, no false
    // swbreak claim. The actual stop is a synthetic
    // pause for GDB attach, not a breakpoint hit.
    protocol::send_packet(
        &mut stream,
        "T05thread:01;",
    )?;

    loop {
        let packet =
            match protocol::recv_packet(&mut stream) {
                Ok(p) => p,
                Err(e) => {
                    if e.kind()
                        == std::io::ErrorKind::UnexpectedEof
                    {
                        break;
                    }
                    continue;
                }
            };

        // Check if this is a resume/step command.
        if let Some(action) =
            check_resume_packet(&packet)
        {
            match action {
                ResumeAction::Continue => {
                    gs.request_resume();
                    let reason =
                        wait_for_stop_with_ctrl_c(
                            gs, &mut stream,
                        )?;
                    let reply = stop_reply(reason);
                    protocol::send_packet(
                        &mut stream, &reply,
                    )?;
                    continue;
                }
                ResumeAction::Step => {
                    gs.request_step();
                    gs.wait_paused();
                    let reason = gs.get_stop_reason();
                    let reply = stop_reply(reason);
                    protocol::send_packet(
                        &mut stream, &reply,
                    )?;
                    continue;
                }
            }
        }

        // Ctrl-C: pause CPU before handler generates
        // stop reply.
        if packet == "\x03" {
            gs.set_stop_reason(StopReason::Pause);
            gs.request_pause();
            gs.wait_paused();
            let reply = stop_reply(StopReason::Pause);
            protocol::send_packet(
                &mut stream, &reply,
            )?;
            continue;
        }

        // All other packets: dispatch to handler.
        let response = match handler
            .handle(&packet, &mut target)
        {
            Some(resp) => resp,
            None => {
                let _ = protocol::send_packet(
                    &mut stream,
                    "OK",
                );
                break;
            }
        };

        if let Err(e) = protocol::send_packet(
            &mut stream,
            &response,
        ) {
            eprintln!("machina: gdb send error: {}", e);
            break;
        }
    }

    gs.detach();
    Ok(())
}
