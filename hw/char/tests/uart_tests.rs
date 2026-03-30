use machina_hw_char::uart::Uart16550;

#[test]
fn test_uart_lsr_initial() {
    let mut uart = Uart16550::new();
    let lsr = uart.read(5);
    // THR empty (bit 5) and transmitter empty (bit 6).
    assert_ne!(lsr & 0x20, 0, "THRE should be set");
    assert_ne!(lsr & 0x40, 0, "TEMT should be set");
}

#[test]
fn test_uart_write_thr() {
    let mut uart = Uart16550::new();
    uart.write(0, 0x41); // write 'A'
    let lsr = uart.read(5);
    // In emulation THR is immediately "sent", so
    // THRE stays set.
    assert_ne!(lsr & 0x20, 0, "THRE should remain set");
}

#[test]
fn test_uart_receive() {
    let mut uart = Uart16550::new();
    uart.receive(0x42); // push 'B'
    let lsr = uart.read(5);
    assert_ne!(lsr & 0x01, 0, "DR should be set");
}

#[test]
fn test_uart_read_rbr() {
    let mut uart = Uart16550::new();
    uart.receive(0x42);
    let ch = uart.read(0);
    assert_eq!(ch, 0x42);

    // After reading, DR should be cleared (FIFO empty).
    let lsr = uart.read(5);
    assert_eq!(lsr & 0x01, 0, "DR should be cleared");
}

#[test]
fn test_uart_dlab() {
    let mut uart = Uart16550::new();

    // Set DLAB.
    uart.write(3, 0x80);

    // Write DLL and DLM.
    uart.write(0, 0x0C); // DLL = 12
    uart.write(1, 0x00); // DLM = 0

    // Read them back.
    assert_eq!(uart.read(0), 0x0C);
    assert_eq!(uart.read(1), 0x00);

    // Clear DLAB, verify normal register access.
    uart.write(3, 0x00);
    // Offset 0 is now RBR (should be 0, no data).
    // Offset 1 is IER.
    uart.write(1, 0x01); // IER = enable RX
    assert_eq!(uart.read(1), 0x01);
}

#[test]
fn test_uart_fifo() {
    let mut uart = Uart16550::new();

    // Push multiple bytes.
    uart.receive(0x61); // 'a'
    uart.receive(0x62); // 'b'
    uart.receive(0x63); // 'c'

    // Read them in order.
    assert_eq!(uart.read(0), 0x61);
    assert_eq!(uart.read(0), 0x62);
    assert_eq!(uart.read(0), 0x63);

    // FIFO empty now.
    let lsr = uart.read(5);
    assert_eq!(lsr & 0x01, 0, "DR should be cleared");
}

#[test]
fn test_uart_irq_on_receive() {
    let mut uart = Uart16550::new();

    // Enable RX available interrupt.
    uart.write(1, 0x01); // IER bit 0

    // No data yet, no IRQ.
    assert!(!uart.irq_pending());

    // Receive a byte — should raise IRQ.
    uart.receive(0x55);
    assert!(uart.irq_pending());

    // IIR should indicate RX data available (0x04).
    let iir = uart.read(2);
    assert_eq!(iir, 0x04);

    // Read the byte — IRQ should clear.
    let _ = uart.read(0);
    assert!(!uart.irq_pending());
}
