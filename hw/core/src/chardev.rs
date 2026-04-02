// Character device backend framework.

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

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

    /// Start delivering input bytes via the callback.
    /// The backend is responsible for how (thread, poll,
    /// etc.). The callback is invoked with each byte.
    fn start_input(&mut self, _cb: Arc<Mutex<dyn FnMut(u8) + Send>>) {}
}

// -- CharFrontend ------------------------------------------------

/// Bridges a device (frontend) to a chardev backend.
pub struct CharFrontend {
    backend: Box<dyn Chardev>,
}

impl CharFrontend {
    pub fn new(backend: Box<dyn Chardev>) -> Self {
        Self { backend }
    }

    /// Write a byte slice to the backend.
    pub fn write(&mut self, data: &[u8]) {
        for &b in data {
            self.backend.write(b);
        }
    }

    /// Start receiving input from the backend. The
    /// callback is invoked for each byte received.
    pub fn start_input(&mut self, cb: Arc<Mutex<dyn FnMut(u8) + Send>>) {
        self.backend.start_input(cb);
    }
}

// -- NullChardev -------------------------------------------------

/// Discards all output and never produces input.
pub struct NullChardev;

impl Chardev for NullChardev {
    fn read(&mut self) -> Option<u8> {
        None
    }

    fn write(&mut self, _data: u8) {}

    fn can_read(&self) -> bool {
        false
    }
}

// -- StdioChardev ------------------------------------------------

/// Wraps host stdin/stdout with QEMU-compatible escape
/// sequences (Ctrl+A prefix):
///   Ctrl+A, X — exit emulator
///   Ctrl+A, H — show help
///   Ctrl+A, Ctrl+A — send literal Ctrl+A
pub struct StdioChardev {
    _thread: Option<std::thread::JoinHandle<()>>,
    saved_termios: Option<libc::termios>,
}

const ESCAPE_CHAR: u8 = 0x01; // Ctrl+A

impl StdioChardev {
    pub fn new() -> Self {
        let saved = enable_raw_mode();
        if saved.is_some() {
            eprintln!(
                "machina: Ctrl+A H for help"
            );
        }
        Self {
            _thread: None,
            saved_termios: saved,
        }
    }
}

impl Default for StdioChardev {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for StdioChardev {
    fn drop(&mut self) {
        if let Some(ref t) = self.saved_termios {
            restore_termios(t);
        }
    }
}

impl Chardev for StdioChardev {
    fn read(&mut self) -> Option<u8> {
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

    fn start_input(
        &mut self,
        cb: Arc<Mutex<dyn FnMut(u8) + Send>>,
    ) {
        let handle = std::thread::spawn(move || {
            use std::io::Read;
            let stdin = std::io::stdin();
            let mut buf = [0u8; 1];
            let mut escape = false;
            loop {
                match stdin.lock().read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let ch = buf[0];
                        if escape {
                            escape = false;
                            match ch {
                                b'x' | b'X' => {
                                    eprintln!(
                                        "\nmachina: \
                                         terminated \
                                         by user"
                                    );
                                    std::process::exit(0);
                                }
                                b'h' | b'H' => {
                                    eprintln!(
                                        "\nCtrl+A H  \
                                         this help\n\
                                         Ctrl+A X  \
                                         exit\n\
                                         Ctrl+A \
                                         Ctrl+A  \
                                         send Ctrl+A"
                                    );
                                }
                                ESCAPE_CHAR => {
                                    if let Ok(mut f) =
                                        cb.lock()
                                    {
                                        f(ESCAPE_CHAR);
                                    }
                                }
                                _ => {
                                    // Unknown: ignore
                                    // the escape and
                                    // pass the char.
                                    if let Ok(mut f) =
                                        cb.lock()
                                    {
                                        f(ch);
                                    }
                                }
                            }
                        } else if ch == ESCAPE_CHAR {
                            escape = true;
                        } else {
                            if let Ok(mut f) =
                                cb.lock()
                            {
                                f(ch);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self._thread = Some(handle);
    }
}

fn enable_raw_mode() -> Option<libc::termios> {
    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut orig) != 0 {
            return None;
        }
        let mut raw = orig;
        // Disable canonical mode, echo, and signals.
        raw.c_lflag &=
            !(libc::ICANON | libc::ECHO | libc::ISIG);
        // Disable input processing (Ctrl+C, Ctrl+S, etc).
        raw.c_iflag &= !(libc::IXON
            | libc::ICRNL
            | libc::INLCR
            | libc::IGNCR);
        // Read 1 byte at a time.
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(0, libc::TCSANOW, &raw) != 0
        {
            return None;
        }
        Some(orig)
    }
}

fn restore_termios(orig: &libc::termios) {
    unsafe {
        libc::tcsetattr(0, libc::TCSANOW, orig);
    }
}

// -- SocketChardev -----------------------------------------------

/// Unix-socket backed chardev (for integration testing).
pub struct SocketChardev {
    stream: Option<UnixStream>,
}

impl SocketChardev {
    pub fn new() -> Self {
        Self { stream: None }
    }

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
}
