use machina_hw_intc::aclint::Aclint;

#[test]
fn test_aclint_mtime_read_write() {
    let mut aclint = Aclint::new(2);

    // mtime at MTIMER offset 0xBFF8, initially 0.
    assert_eq!(aclint.mtimer_read(0xBFF8, 8), 0);

    // Write mtime.
    aclint.mtimer_write(0xBFF8, 8, 1000);
    assert_eq!(aclint.mtimer_read(0xBFF8, 8), 1000);
}

#[test]
fn test_aclint_mtimecmp_set() {
    let mut aclint = Aclint::new(2);

    // mtimecmp[0] at offset 0x0000.
    aclint.mtimer_write(0x0000, 8, 500);
    assert_eq!(aclint.mtimer_read(0x0000, 8), 500);

    // mtimecmp[1] at offset 0x0008.
    aclint.mtimer_write(0x0008, 8, 1000);
    assert_eq!(aclint.mtimer_read(0x0008, 8), 1000);
}

#[test]
fn test_aclint_msip_set_clear() {
    let mut aclint = Aclint::new(2);

    // msip[0] at MSWI offset 0x0000.
    aclint.mswi_write(0x0000, 4, 1);
    assert_eq!(aclint.mswi_read(0x0000, 4), 1);

    // Clear.
    aclint.mswi_write(0x0000, 4, 0);
    assert_eq!(aclint.mswi_read(0x0000, 4), 0);

    // Only bit 0 is writable.
    aclint.mswi_write(0x0000, 4, 0xFF);
    assert_eq!(aclint.mswi_read(0x0000, 4), 1);
}

#[test]
fn test_aclint_timer_compare() {
    let mut aclint = Aclint::new(2);

    // Set mtimecmp[0] = 5.
    aclint.mtimer_write(0x0000, 8, 5);

    // Initially no pending.
    assert!(!aclint.timer_irq_pending(0));

    // Tick 4 times -> mtime = 4, still < 5.
    for _ in 0..4 {
        aclint.tick();
    }
    assert!(!aclint.timer_irq_pending(0));

    // Tick once more -> mtime = 5, now >= mtimecmp.
    aclint.tick();
    assert!(aclint.timer_irq_pending(0));

    // Hart 1 has mtimecmp = u64::MAX, still not pending.
    assert!(!aclint.timer_irq_pending(1));
}
