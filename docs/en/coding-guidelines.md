# Machina Coding Guidelines

General coding guidelines for the Machina project. For Rust-specific rules,
see [Rust Guidelines](rust-guidelines.md). For formatting and naming
conventions, see [Coding Style](coding-style.md).

## Naming

### Be descriptive

No single-letter names or ambiguous abbreviations. Names should reveal intent.

```rust
// Bad
let v = t.read();
let p = addr >> 12;

// Good
let value = temp.read();
let page_number = addr >> PAGE_SHIFT;
```

### Be accurate

Names must match what the code actually does. If the name says "count", the
value should be a count -- not an index, offset, or mask.

### Encode units in names

When a value carries a physical unit or scale, embed it in the name.

```rust
let timeout_ms = 5000;
let frame_size_in_pages = 4;
let clock_freq_hz = 12_000_000;
```

### Boolean naming

Use assertion-style names for booleans: `is_*`, `has_*`, `can_*`, `should_*`.

```rust
let is_kernel_mode = mode == Prv::M;
let has_side_effects = op.flags().contains(OpFlags::SIDE_EFFECT);
```

## Comments

### Explain why, not what

Comments that restate code are noise. Explain the reasoning behind non-obvious
decisions.

```rust
// Bad: restates the code
// Check if page is present
if pte.flags().contains(PteFlags::V) { ... }

// Good: explains the constraint
// Sv39 spec: fetch falls through when V=0 in S-mode
// only traps in M-mode (spec 4.3.1)
if pte.flags().contains(PteFlags::V) { ... }
```

### Document design decisions

When multiple approaches exist, record why this one was chosen. Future readers
need to understand the trade-off, not just the outcome.

### Cite specifications

When implementing hardware behavior, cite the specification section.

```rust
// RISC-V Privileged Spec 4.3.1 -- PTE attribute for global mappings
const PTE_G: u64 = 0x10;
```

## File Organization

### One concept per file

Split files when they grow long or mix unrelated responsibilities. A file
named `mmu.rs` should not contain interrupt handling logic.

### Organize for top-down reading

Place high-level entry points first. Helper functions and internal details
follow. A reader should understand the public API by reading the first section.

### Group into logical paragraphs

Within a function, group related statements together. Separate groups with a
blank line. Each group should express one step in the algorithm.

## API Design

### Hide implementation details

Default to the narrowest visibility. Expose only what callers need.

```rust
// Prefer
pub(crate) fn translate_one(ctx: &mut Context) { ... }

// Avoid
pub fn translate_one(ctx: &mut Context) { ... }
```

### Validate at boundaries, trust internally

Validate inputs at public API boundaries (e.g., syscall entry, device MMIO
write). Inside the crate, trust already-validated values.

### Use types to enforce invariants

If a value has constraints, encode them in the type system rather than checking
at every use site.

```rust
// Prefer: invalid states are unrepresentable
pub struct PhysicalPage(u64);

impl PhysicalPage {
    pub fn new(frame: u64) -> Option<Self> {
        (frame < MAX_PHYS_PAGE).then_some(PhysicalPage(frame))
    }
}

// Avoid: raw u64 could be any value
fn map_page(frame: u64) { ... }
```

## Error Messages

Format error messages consistently. Include the operation that failed, the
value or identifier involved, and (where applicable) the expected range.

```
"invalid PTE at {vpn}: reserved bits set"
"out of TB cache: capacity {cap}, requested {size}"
```
