# Machina Rust Guidelines

Rust-specific guidelines for the Machina project. For general coding rules, see
[Coding Guidelines](coding-guidelines.md). For formatting conventions, see
[Coding Style](coding-style.md).

## Unsafe Rust

### Justify every use of unsafe

Every `unsafe` block requires a `// SAFETY:` comment explaining why the
operation is sound. Every `unsafe fn` or `unsafe trait` requires a `# Safety`
doc section describing the conditions callers must uphold.

```rust
// SAFETY: buf points to a valid, RWX-mapped region of `len` bytes.
// The mmap call above guaranteed alignment and permissions.
unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), buf, len) }
```

### Unsafe is confined to specific modules

`unsafe` is only permitted for JIT code buffer management, function pointer
casts for generated code, raw pointer access in the TLB fast path, inline
assembly in the backend emitter, and FFI. All other code must be safe Rust.

## Functions

### Keep functions small and focused

A function should do one thing. If it needs a comment to separate sections,
consider splitting it.

### Minimize nesting

Target at most 3 levels of nesting. Use early returns, `let...else`, and `?`
to flatten control flow.

```rust
// Prefer
let Some(pte) = page_table.walk(vpn) else {
    return Err(MmuFault::InvalidPte);
};

// Avoid
if let Some(pte) = page_table.walk(vpn) {
    // ... deeply nested logic ...
}
```

### Avoid boolean parameters

Use an enum or split into two functions.

```rust
// Prefer
pub fn emit_load_signed(buf: &mut CodeBuffer, ...) { ... }
pub fn emit_load_unsigned(buf: &mut CodeBuffer, ...) { ... }

// Avoid
pub fn emit_load(buf: &mut CodeBuffer, signed: bool, ...) { ... }
```

## Types and Traits

### Prefer enums over trait objects for closed sets

When the set of variants is known at compile time, use an enum. Trait objects
(`dyn Trait`) are appropriate only for open-ended extensibility.

```rust
// Prefer
enum Exception {
    InstructionAccessFault,
    LoadAccessFault,
    StoreAccessFault,
    // ...
}

// Avoid
trait Exception { fn handle(&self); }
```

### Encapsulate fields behind getters

Expose fields through methods rather than making them `pub`. This preserves
the ability to add validation or logging later.

## Modules and Crates

### Default to narrow visibility

Use `pub(super)` or `pub(crate)` by default. Only use `pub` when external
crates genuinely need access.

### Qualify function imports via parent module

Import the parent module, then call the function through it. This makes the
origin explicit.

```rust
// Prefer
use core::mem;
mem::replace(&mut slot, new_value)

// Avoid
use core::mem::replace;
replace(&mut slot, new_value)
```

### Use workspace dependencies

All shared dependency versions must be declared in the workspace root
`Cargo.toml` under `[workspace.dependencies]`. Individual crates reference
them with `{ workspace = true }`.

## Error Handling

### Propagate errors with `?`

Do not `.unwrap()` or `.expect()` where failure is possible. Use `?` to
propagate errors to the caller.

### Define domain error types

Use dedicated error enums rather than generic `String` or `Box<dyn Error>`.

```rust
#[derive(Debug)]
enum TranslateError {
    InvalidOpcode(u32),
    UnsupportedExtension(char),
    BufferOverflow { requested: usize, available: usize },
}
```

## Concurrency

### Document lock ordering

When multiple locks exist, document the acquisition order and follow it
consistently to prevent deadlocks.

### No I/O under spinlock

Never perform I/O or blocking operations while holding a spinlock. This
includes memory allocation and print statements.

### Avoid casual atomics

`Ordering` is subtle. Use `SeqCst` by default. Only relax to `Acquire`/
`Release`/`Relaxed` when there is a documented performance reason and the
correctness argument is written in a comment.

## Performance

### No O(n) on hot paths

The translation fast path (TB lookup, code execution, TLB walk) must avoid
O(n) operations. Use hash tables, direct-mapped caches, or indexed arrays.

### Minimize unnecessary copies

Pass large structures by reference. Use `&[u8]` instead of `Vec<u8>` when
ownership is not needed. Avoid cloning `Arc` on every iteration.

### No premature optimization

Optimization commits must include a benchmark showing the improvement.

## Macros and Attributes

### Prefer functions over macros

Use a macro only when a function cannot do the job (e.g., repeating
declarations, generating match arms).

### Suppress lints at narrowest scope

Apply `#[allow(...)]` or `#[expect(...)]` to the specific item, not the entire
module.

### Sort derive traits alphabetically

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
```

### Workspace lints

Every workspace member must include:

```toml
[lints]
workspace = true
```
