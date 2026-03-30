use machina_hw_intc::plic::Plic;

#[test]
fn test_plic_set_priority() {
    let mut plic = Plic::new(64, 2);

    // Write priority for IRQ 1 via MMIO (offset 4).
    plic.write(0x04, 4, 7);
    assert_eq!(plic.read(0x04, 4), 7);

    // Write priority for IRQ 10 (offset 0x28).
    plic.write(0x28, 4, 3);
    assert_eq!(plic.read(0x28, 4), 3);
}

#[test]
fn test_plic_claim_highest() {
    let mut plic = Plic::new(64, 2);

    // Set priorities: IRQ 1 = 2, IRQ 2 = 5.
    plic.write(0x04, 4, 2); // priority[1] = 2
    plic.write(0x08, 4, 5); // priority[2] = 5

    // Enable IRQ 1 and IRQ 2 for context 0.
    // Enable bitmap at 0x2000 + 0x80*0 = 0x2000.
    // Both IRQs are in word 0: bits 1 and 2.
    plic.write(0x2000, 4, 0x06);

    // Set both pending.
    plic.set_pending(1, true);
    plic.set_pending(2, true);

    // Claim should return IRQ 2 (higher priority).
    let claimed = plic.claim_irq(0);
    assert_eq!(claimed, Some(2));
}

#[test]
fn test_plic_complete() {
    let mut plic = Plic::new(64, 1);

    plic.write(0x04, 4, 1); // priority[1] = 1
    plic.write(0x2000, 4, 0x02); // enable IRQ 1, ctx 0
    plic.set_pending(1, true);

    let claimed = plic.claim_irq(0);
    assert_eq!(claimed, Some(1));

    // Complete via MMIO: write IRQ number to
    // claim/complete register at 0x200004.
    plic.write(0x200004, 4, 1);

    // After completion, claim register should be 0.
    assert_eq!(plic.read(0x200004, 4), 0);
}

#[test]
fn test_plic_threshold() {
    let mut plic = Plic::new(64, 1);

    plic.write(0x04, 4, 3); // priority[1] = 3
    plic.write(0x2000, 4, 0x02); // enable IRQ 1, ctx 0
    plic.set_pending(1, true);

    // Set threshold for context 0 to 5 (above priority).
    plic.write(0x200000, 4, 5);

    // IRQ 1 has priority 3 which is <= threshold 5,
    // so it should not be claimable.
    assert_eq!(plic.claim_irq(0), None);
}

#[test]
fn test_plic_no_pending() {
    let mut plic = Plic::new(64, 1);

    plic.write(0x04, 4, 1); // priority[1] = 1
    plic.write(0x2000, 4, 0x02); // enable IRQ 1, ctx 0

    // Nothing pending.
    assert_eq!(plic.claim_irq(0), None);
}
