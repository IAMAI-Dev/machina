# Machina Git Guidelines

Git commit and pull request conventions for the Machina project.

## Commit Messages

### Format

```
module: subject

Body describing what changed and why.

Signed-off-by: Name <email>
```

### Subject line

- Format: `module: subject`
- Imperative mood: `add`, `fix`, `remove` -- not `added`, `fixed`, `removed`
- Lowercase subject, no trailing period
- Total length at or below 72 characters

### Common module prefixes

| Module | Scope |
|--------|-------|
| `core` | IR types, opcodes, CPU trait |
| `accel` | IR optimization, regalloc, codegen, exec engine |
| `guest/riscv` | RISC-V frontend (decode, translate) |
| `decode` | .decode parser and codegen |
| `system` | CPU manager, GDB bridge, WFI |
| `memory` | AddressSpace, MMIO, RAM blocks |
| `hw/core` | Device infrastructure (qdev, IRQ, FDT) |
| `hw/intc` | PLIC, ACLINT |
| `hw/char` | UART |
| `hw/riscv` | Reference machine, boot, SBI |
| `hw/virtio` | VirtIO MMIO transport and devices |
| `monitor` | QMP/HMP console |
| `gdbstub` | GDB remote protocol |
| `difftest` | Difftest client |
| `tests` | Test suite |
| `docs` | Documentation |
| `project` | Cross-cutting changes (CI, Makefile, configs) |

### Common verb prefixes

| Verb | Usage |
|------|-------|
| `Fix` | Correct a bug |
| `Add` | Introduce new functionality |
| `Remove` | Delete code or features |
| `Refactor` | Restructure without changing behavior |
| `Rename` | Change names of files, modules, or symbols |
| `Implement` | Add a new subsystem or feature |
| `Enable` | Turn on a previously disabled capability |
| `Clean up` | Minor tidying without functional change |
| `Bump` | Update a dependency version |

### Body

- Separated from subject by a blank line
- Describe what changed and why -- not how
- Each line at or below 80 characters

### Examples

```
accel: fix register clobber in div/rem helpers

The x86-64 backend used RDX as a scratch register for division
without saving the guest's original value. Add save/restore around
the DIV instruction.

Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

```
guest/riscv: implement Zbs (single-bit operations)

Add bclr, bset, binv, bext for both register and immediate forms.
The decoder now recognizes the Zbs extension when enabled in
misa.

Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

## Atomic Commits

### One commit, one logical change

Each commit must do exactly one thing. Do not mix unrelated changes in a single
commit. If you find yourself writing "and also" in the commit message, split it.

### Every commit must compile and pass tests

The tree must be in a working state after every commit. No broken intermediate
states. This ensures `git bisect` always works.

### Squash fixup commits before submitting

When reviewing your own branch before opening a PR, squash any temporary
commits into the commit they belong to. Temporary commits include:

- Fixup commits that correct a typo or bug introduced earlier in the branch
- Adjustment commits that tweak a previous change (e.g., renaming, reordering)
- Any commit whose message starts with `fixup!` or `squash!`

Use `git rebase -i` to fold these into the original commit. The final PR
history should read as a clean sequence of logical changes, not a development
journal.

### Separate refactoring from features

If a feature requires preparatory refactoring, put the refactoring in its own
commit(s) before the feature commit. This makes each commit easier to review
and bisect.

## Signed-off-by

All commits in this repository must include a `Signed-off-by` line:

```
Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

Do not add AI-related sign-off lines (e.g. `Co-Authored-By: Claude`).

## Pull Requests

### Keep PRs focused

One topic per PR. A PR that mixes a bug fix, a refactoring, and a new feature
is difficult to review.

### CI must pass

Ensure all CI checks pass before requesting review:

- `make test` -- all tests pass
- `make clippy` -- zero warnings
- `make fmt-check` -- formatting is clean

### Reference issues

When a PR addresses an issue, reference it in the description:

```
Closes #42
```
