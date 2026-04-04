// GDB RSP command handler.
//
// Dispatches incoming GDB packets to the target backend
// and returns the response string.

use crate::protocol;

/// Reason the target stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// Stopped by software breakpoint (SIGTRAP).
    Breakpoint,
    /// Stopped by single-step (SIGTRAP).
    Step,
    /// Stopped by GDB pause request (SIGTRAP).
    Pause,
    /// Target terminated.
    Terminated,
}

/// Trait for GDB target operations.
/// Implemented by the CPU/system layer.
pub trait GdbTarget: Send {
    /// Read all registers as a byte vector.
    fn read_registers(&self) -> Vec<u8>;
    /// Write all registers from a byte slice. Returns true on success.
    fn write_registers(&mut self, _data: &[u8]) -> bool {
        false
    }
    /// Read a single register by number.
    fn read_register(&self, _reg: usize) -> Vec<u8>;
    /// Write a single register by number. Returns true on success.
    fn write_register(&mut self, _reg: usize, _val: &[u8]) -> bool {
        false
    }
    /// Read memory at `addr`, `len` bytes.
    fn read_memory(&self, addr: u64, len: usize) -> Vec<u8>;
    /// Write memory at `addr`. Returns true on success.
    fn write_memory(&mut self, addr: u64, data: &[u8]) -> bool;
    /// Set a breakpoint. `type_`: 0=sw, 1=hw, 2=write, 3=read, 4=access.
    fn set_breakpoint(&mut self, type_: u8, addr: u64, kind: u32) -> bool;
    /// Remove a breakpoint.
    fn remove_breakpoint(&mut self, type_: u8, addr: u64, kind: u32) -> bool;
    /// Resume target execution.
    fn resume(&mut self);
    /// Single-step the target.
    fn step(&mut self);
    /// Get current PC.
    fn get_pc(&self) -> u64;
    /// Get the reason for the current stop.
    fn get_stop_reason(&self) -> StopReason;
}

/// GDB command handler. Processes one packet at a time.
pub struct GdbHandler {
    no_ack: bool,
    attached: bool,
    /// XML target description for qXfer.
    target_xml: &'static str,
}

impl GdbHandler {
    pub fn new() -> Self {
        Self::with_target_xml(crate::target::RISCV64_TARGET_XML)
    }

    /// Create a handler with a custom target XML description.
    pub fn with_target_xml(xml: &'static str) -> Self {
        Self {
            no_ack: false,
            attached: true,
            target_xml: xml,
        }
    }

    /// Process an incoming packet and return the response.
    /// Returns None to indicate the session should end.
    pub fn handle(
        &mut self,
        packet: &str,
        target: &mut dyn GdbTarget,
    ) -> Option<String> {
        if packet == "\x03" {
            // Ctrl-C interrupt: pause target.
            return Some(self.stop_reply(StopReason::Pause));
        }

        // Special case: 'v' packets (vCont, vMustReplyEmpty)
        // use the entire packet as command with no split.
        if packet.starts_with('v') {
            return self.handle_v_packet(packet, target);
        }

        // Special case: 'q'/'Q' packets — split at the first
        // non-alphabetic char to get the query name.
        if packet.starts_with('q') {
            let (name, args) = match packet.find(|c: char| !c.is_alphabetic()) {
                Some(i) => (&packet[..i], &packet[i..]),
                None => (packet, ""),
            };
            let resp = self.handle_query(name, args, target);
            return Some(resp);
        }
        if packet.starts_with('Q') {
            let (name, args) = match packet.find(|c: char| !c.is_alphabetic()) {
                Some(i) => (&packet[..i], &packet[i..]),
                None => (packet, ""),
            };
            let _ = args; // unused for now
            let resp = self.handle_set(name);
            return Some(resp);
        }

        let (cmd, args) = match packet.chars().next() {
            Some('?') => ("?", &packet[1..]),
            Some(c) if c.is_ascii_uppercase() => {
                // Single uppercase command letter, rest is args.
                (&packet[..1], &packet[1..])
            }
            Some(c) if c.is_ascii_lowercase() => {
                // Single lowercase command letter, rest is args.
                (&packet[..1], &packet[1..])
            }
            _ => (packet, ""),
        };

        let resp = match cmd {
            "?" => self.handle_stop_reason(target),
            "g" => self.handle_read_registers(target),
            "G" => self.handle_write_registers(target, args),
            "p" => self.handle_read_register(target, args),
            "P" => self.handle_write_register(target, args),
            "m" => self.handle_read_memory(target, args),
            "M" => self.handle_write_memory(target, args),
            "X" => self.handle_write_memory_binary(target, args),
            "c" => return self.handle_continue(target),
            "s" => return self.handle_step(target),
            "Z" => self.handle_set_breakpoint(target, args),
            "z" => self.handle_remove_breakpoint(target, args),
            "D" => {
                self.attached = false;
                return None;
            }
            "k" => return None,
            "H" => "OK".to_string(),
            "T" => "OK".to_string(),
            _ => String::new(),
        };

        Some(resp)
    }

    fn handle_stop_reason(&self, target: &mut dyn GdbTarget) -> String {
        self.stop_reply(target.get_stop_reason())
    }

    fn stop_reply(&self, reason: StopReason) -> String {
        match reason {
            StopReason::Breakpoint => {
                "T05thread:01;swbreak:;".to_string()
            }
            StopReason::Step => "S05".to_string(),
            StopReason::Pause => "T02thread:01;".to_string(),
            StopReason::Terminated => "W00".to_string(),
        }
    }

    fn handle_read_registers(
        &self,
        target: &mut dyn GdbTarget,
    ) -> String {
        let data = target.read_registers();
        protocol::encode_hex_bytes(&data)
    }

    fn handle_write_registers(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        match protocol::decode_hex_bytes(args) {
            Ok(data) => {
                if target.write_registers(&data) {
                    "OK".to_string()
                } else {
                    "E01".to_string()
                }
            }
            Err(_) => "E01".to_string(),
        }
    }

    fn handle_read_register(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        let reg = protocol::parse_hex(args.trim_start_matches(':')) as usize;
        let data = target.read_register(reg);
        protocol::encode_hex_bytes(&data)
    }

    fn handle_write_register(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        let parts: Vec<&str> = args.splitn(2, '=').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }
        let reg = protocol::parse_hex(parts[0]) as usize;
        match protocol::decode_hex_bytes(parts[1]) {
            Ok(data) => {
                if target.write_register(reg, &data) {
                    "OK".to_string()
                } else {
                    "E01".to_string()
                }
            }
            Err(_) => "E01".to_string(),
        }
    }

    fn handle_read_memory(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        let parts: Vec<&str> = args.splitn(2, ',').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }
        let addr = protocol::parse_hex(parts[0]);
        let len = protocol::parse_hex(parts[1]) as usize;
        let data = target.read_memory(addr, len);
        protocol::encode_hex_bytes(&data)
    }

    fn handle_write_memory(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        // Maddr,length:hexdata
        let colon = match args.find(':') {
            Some(i) => i,
            None => return "E01".to_string(),
        };
        let header = &args[..colon];
        let data_hex = &args[colon + 1..];
        let parts: Vec<&str> = header.splitn(2, ',').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }
        let addr = protocol::parse_hex(parts[0]);
        match protocol::decode_hex_bytes(data_hex) {
            Ok(data) => {
                if target.write_memory(addr, &data) {
                    "OK".to_string()
                } else {
                    "E01".to_string()
                }
            }
            Err(_) => "E01".to_string(),
        }
    }

    fn handle_write_memory_binary(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        // X addr,length:binary_data
        let colon = match args.find(':') {
            Some(i) => i,
            None => return "E01".to_string(),
        };
        let header = &args[..colon];
        let data = &args[colon + 1..];
        let parts: Vec<&str> = header.splitn(2, ',').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }
        let addr = protocol::parse_hex(parts[0]);
        // Binary data may contain escaped bytes (#$} are escaped
        // as }XOR 0x20).
        let unescaped = unescape_binary(data.as_bytes());
        if target.write_memory(addr, &unescaped) {
            "OK".to_string()
        } else {
            "E01".to_string()
        }
    }

    fn handle_continue(
        &mut self,
        target: &mut dyn GdbTarget,
    ) -> Option<String> {
        target.resume();
        // resume blocks until CPU stops.
        Some(self.stop_reply(target.get_stop_reason()))
    }

    fn handle_step(
        &mut self,
        target: &mut dyn GdbTarget,
    ) -> Option<String> {
        target.step();
        // step blocks until step completes.
        Some(self.stop_reply(StopReason::Step))
    }

    fn handle_set_breakpoint(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        let parts: Vec<&str> = args.splitn(3, ',').collect();
        if parts.len() < 2 {
            return "E01".to_string();
        }
        let type_ = parts[0].parse::<u8>().unwrap_or(0);
        let addr = protocol::parse_hex(parts[1]);
        let kind = parts
            .get(2)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(4);
        if target.set_breakpoint(type_, addr, kind) {
            "OK".to_string()
        } else {
            String::new()
        }
    }

    fn handle_remove_breakpoint(
        &self,
        target: &mut dyn GdbTarget,
        args: &str,
    ) -> String {
        let parts: Vec<&str> = args.splitn(3, ',').collect();
        if parts.len() < 2 {
            return "E01".to_string();
        }
        let type_ = parts[0].parse::<u8>().unwrap_or(0);
        let addr = protocol::parse_hex(parts[1]);
        let kind = parts
            .get(2)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(4);
        if target.remove_breakpoint(type_, addr, kind) {
            "OK".to_string()
        } else {
            String::new()
        }
    }

    fn handle_query(
        &mut self,
        name: &str,
        args: &str,
        _target: &mut dyn GdbTarget,
    ) -> String {
        match name {
            "qSupported" => {
                "multiprocess+;vContSupported+;QStartNoAckMode+;\
                 PacketSize=4000;qXfer:features:read+"
                    .to_string()
            }
            "qAttached" => {
                if self.attached { "1" } else { "0" }
                    .to_string()
            }
            "qC" => "QC01".to_string(),
            "qfThreadInfo" => "m01".to_string(),
            "qsThreadInfo" => "l".to_string(),
            "qOffsets" => String::new(),
            _ if name.starts_with("qSymbol") => {
                "OK".to_string()
            }
            _ if name.starts_with("qThreadExtraInfo") => {
                "6d616368696e61".to_string() // "machina"
            }
            _ if name.starts_with("qXfer") => {
                self.handle_qxfer(args)
            }
            _ => String::new(),
        }
    }

    fn handle_qxfer(&self, args: &str) -> String {
        // args = ":features:read:target.xml:offset,length"
        let parts: Vec<&str> = args.split(':').collect();
        if parts.len() < 5 {
            return String::new();
        }
        let object = parts[1]; // "features"
        let action = parts[2]; // "read"
        let annex = parts[3]; // "target.xml"
        let range = parts[4]; // "offset,length"

        if object != "features" || action != "read" {
            return String::new();
        }
        if annex != "target.xml" {
            return String::new();
        }

        let range_parts: Vec<&str> = range.split(',').collect();
        if range_parts.len() != 2 {
            return String::new();
        }
        let offset = protocol::parse_hex(range_parts[0]) as usize;
        let length = protocol::parse_hex(range_parts[1]) as usize;

        let xml = self.target_xml;
        if offset >= xml.len() {
            return "l".to_string();
        }
        let end = (offset + length).min(xml.len());
        let data = &xml.as_bytes()[offset..end];
        let mut resp = String::with_capacity(data.len() + 1);
        if offset + data.len() < xml.len() {
            resp.push('m');
        } else {
            resp.push('l');
        }
        resp.push_str(
            std::str::from_utf8(data).unwrap_or(""),
        );
        resp
    }

    fn handle_set(&mut self, name: &str) -> String {
        match name {
            "QStartNoAckMode" => {
                self.no_ack = true;
                "OK".to_string()
            }
            _ => String::new(),
        }
    }

    fn handle_v_packet(
        &mut self,
        packet: &str,
        target: &mut dyn GdbTarget,
    ) -> Option<String> {
        if packet == "vCont?" {
            return Some("vCont;c;C;s;S".to_string());
        }
        if let Some(rest) = packet.strip_prefix("vCont") {
            if rest.is_empty() || rest == ";" {
                return Some("OK".to_string());
            }
            return self.handle_v_cont(rest, target);
        }
        if packet.starts_with("vMustReplyEmpty") {
            return Some(String::new());
        }
        Some(String::new())
    }

    fn handle_v_cont(
        &mut self,
        args: &str,
        target: &mut dyn GdbTarget,
    ) -> Option<String> {
        // vCont;action[:thread];action[:thread]...
        // For single-thread, just use the first action.
        let actions: Vec<&str> = args.trim_start_matches(';').split(';').collect();
        let action = actions.first()?;

        let (cmd, _thread): (&str, &str) =
            match action.find(':') {
                Some(i) => (&action[..i], &action[i + 1..]),
                None => (*action, ""),
            };

        match cmd {
            "c" | "C" => self.handle_continue(target),
            "s" | "S" => self.handle_step(target),
            _ => Some(String::new()),
        }
    }

    pub fn no_ack(&self) -> bool {
        self.no_ack
    }
}

/// Unescape GDB binary data (}XOR escaping).
fn unescape_binary(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'}' && i + 1 < data.len() {
            out.push(data[i + 1] ^ 0x20);
            i += 2;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

impl Default for GdbHandler {
    fn default() -> Self {
        Self::new()
    }
}
