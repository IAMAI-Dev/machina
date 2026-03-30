// Character device backend framework.

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

type CharHandler = Mutex<Option<Box<dyn FnMut(u8) + Send>>>;

/// Trait for character device backends.
///
/// A chardev provides byte-level I/O used by serial ports,
/// consoles, and similar devices.
pub trait Chardev: Send {
    /// Read one byte if available.
    fn read(&mut self) -> Option<u8>;

    /// Write one byte to the backend.
    fn write(&mut self, data: u8);

    /// Returns `true` if data is available to read.
    fn can_read(&self) -> bool;

    /// Install (or clear) an input handler invoked when the
    /// backend receives data from the outside world.
    fn set_handler(&mut self, handler: Option<Box<dyn FnMut(u8) + Send>>);
}

// -- CharEvent / CharFrontend -------------------------------------

/// Lifecycle events delivered to a frontend.
#[derive(Debug, Clone, Copy)]
pub enum CharEvent {
    Opened,
    Closed,
}

/// Callback invoked when the backend delivers data.
pub type CharReceiveHandler = Box<dyn FnMut(&[u8]) + Send>;

/// Callback invoked on backend lifecycle events.
pub type CharEventHandler = Box<dyn FnMut(CharEvent) + Send>;

/// Bridges a device (frontend) to a chardev backend.
pub struct CharFrontend {
    backend: Box<dyn Chardev>,
    receive_handler: Option<CharReceiveHandler>,
    event_handler: Option<CharEventHandler>,
}

impl CharFrontend {
    pub fn new(backend: Box<dyn Chardev>) -> Self {
        Self {
            backend,
            receive_handler: None,
            event_handler: None,
        }
    }

    pub fn set_handlers(
        &mut self,
        recv: CharReceiveHandler,
        event: CharEventHandler,
    ) {
        self.receive_handler = Some(recv);
        self.event_handler = Some(event);
    }

    /// Write a byte slice to the backend, one byte at a time.
    pub fn write(&mut self, data: &[u8]) {
        for &b in data {
            self.backend.write(b);
        }
    }

    /// Poll the backend for available data and forward it to
    /// the receive handler.
    pub fn poll(&mut self) {
        if !self.backend.can_read() {
            return;
        }
        let mut buf = Vec::new();
        while self.backend.can_read() {
            if let Some(b) = self.backend.read() {
                buf.push(b);
            } else {
                break;
            }
        }
        if !buf.is_empty() {
            if let Some(ref mut handler) = self.receive_handler {
                handler(&buf);
            }
        }
    }
}

// -- NullChardev ---------------------------------------------------

/// Discards all output and never produces input.
pub struct NullChardev;

impl Chardev for NullChardev {
    fn read(&mut self) -> Option<u8> {
        None
    }

    fn write(&mut self, _data: u8) {
        // Discard silently.
    }

    fn can_read(&self) -> bool {
        false
    }

    fn set_handler(&mut self, _handler: Option<Box<dyn FnMut(u8) + Send>>) {
        // Nothing to do — null backend never delivers input.
    }
}

// -- StdioChardev --------------------------------------------------

/// Wraps host stdin/stdout.
pub struct StdioChardev {
    handler: CharHandler,
}

impl StdioChardev {
    pub fn new() -> Self {
        Self {
            handler: Mutex::new(None),
        }
    }
}

impl Default for StdioChardev {
    fn default() -> Self {
        Self::new()
    }
}

impl Chardev for StdioChardev {
    fn read(&mut self) -> Option<u8> {
        // Non-blocking stdin is platform-specific; leave as
        // None for now.
        None
    }

    fn write(&mut self, data: u8) {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(&[data]);
        let _ = out.flush();
    }

    fn can_read(&self) -> bool {
        false
    }

    fn set_handler(&mut self, handler: Option<Box<dyn FnMut(u8) + Send>>) {
        *self.handler.lock().unwrap() = handler;
    }
}

// -- SocketChardev -------------------------------------------------

/// Unix-socket backed chardev (for integration testing).
pub struct SocketChardev {
    handler: CharHandler,
    stream: Option<UnixStream>,
}

impl SocketChardev {
    pub fn new() -> Self {
        Self {
            handler: Mutex::new(None),
            stream: None,
        }
    }

    /// Connect to a Unix domain socket at `path`.
    pub fn connect(&mut self, path: &str) -> std::io::Result<()> {
        let s = UnixStream::connect(path)?;
        s.set_nonblocking(true)?;
        self.stream = Some(s);
        Ok(())
    }
}

impl Default for SocketChardev {
    fn default() -> Self {
        Self::new()
    }
}

impl Chardev for SocketChardev {
    fn read(&mut self) -> Option<u8> {
        use std::io::Read;
        let stream = self.stream.as_mut()?;
        let mut buf = [0u8; 1];
        match stream.read(&mut buf) {
            Ok(1) => Some(buf[0]),
            _ => None,
        }
    }

    fn write(&mut self, data: u8) {
        if let Some(ref mut stream) = self.stream {
            let _ = stream.write_all(&[data]);
        }
    }

    fn can_read(&self) -> bool {
        self.stream.is_some()
    }

    fn set_handler(&mut self, handler: Option<Box<dyn FnMut(u8) + Send>>) {
        *self.handler.lock().unwrap() = handler;
    }
}
