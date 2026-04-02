// MonitorService: shared backend for MMP and HMP.
//
// Holds references to the VM control plane and
// dispatches commands to pause/resume/query the vCPU.

use std::sync::Arc;

use machina_core::monitor::{MonitorState, VmState};

/// CPU register snapshot (taken while paused).
pub struct CpuSnapshot {
    pub gpr: [u64; 32],
    pub pc: u64,
    pub priv_level: u8,
    pub halted: bool,
}

/// Callback to read CPU state. Set by the main binary
/// after creating FullSystemCpu.
pub type CpuSnapshotFn =
    Box<dyn Fn() -> CpuSnapshot + Send + Sync>;

/// Central monitor service shared by all transports.
pub struct MonitorService {
    pub state: Arc<MonitorState>,
    cpu_snapshot: Option<Arc<CpuSnapshotFn>>,
}

impl MonitorService {
    pub fn new(state: Arc<MonitorState>) -> Self {
        Self {
            state,
            cpu_snapshot: None,
        }
    }

    pub fn set_cpu_snapshot(
        &mut self,
        f: Arc<CpuSnapshotFn>,
    ) {
        self.cpu_snapshot = Some(f);
    }

    pub fn query_status(&self) -> bool {
        self.state.vm_state() == VmState::Running
    }

    pub fn stop(&self) {
        self.state.request_stop();
    }

    pub fn cont(&self) {
        self.state.request_cont();
    }

    pub fn quit(&self) {
        self.state.request_quit();
    }

    pub fn query_cpus(&self) -> Vec<CpuInfo> {
        let snap = self.take_snapshot();
        vec![CpuInfo {
            cpu_index: 0,
            pc: snap.as_ref().map(|s| s.pc).unwrap_or(0),
            halted: snap
                .as_ref()
                .map(|s| s.halted)
                .unwrap_or(false),
            arch: "riscv64".to_string(),
        }]
    }

    pub fn take_snapshot(&self) -> Option<CpuSnapshot> {
        self.cpu_snapshot.as_ref().map(|f| f())
    }
}

pub struct CpuInfo {
    pub cpu_index: u32,
    pub pc: u64,
    pub halted: bool,
    pub arch: String,
}
