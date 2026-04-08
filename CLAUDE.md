# CLAUDE.md

This file defines how Claude Code should work in this repository.

## Core Principle

- Follow a spec-first workflow for any non-trivial feature, behavior change,
  or refactor.
- Treat the spec as the contract. Implementation may vary, but externally
  visible behavior must match the spec.
- Prefer correctness, reproducibility, and narrow changes over speed or
  novelty.

## Before Writing Code

- Read the relevant code and tests first. Do not guess hidden invariants.
- If the request changes behavior in a meaningful way, work from a written
  spec before editing code.
- If no spec exists for non-trivial work, draft a minimal one or ask for one
  before implementation.
- A useful spec should define the goal, interface, expected behavior, edge
  cases, failure handling, and a concrete test plan.
- When matching external behavior, verify against the authoritative upstream
  source and record that reference in the spec or change notes.

## Change Scope

- Keep the change boundary as small as possible.
- Do not perform opportunistic refactors unless they are required for
  correctness, safety, or testability.
- Prefer removing dead or obsolete code over preserving unused paths.
- Follow existing local patterns unless the spec requires a deliberate change.

## Implementation Rules

- Prefer simple, explicit code over clever abstractions.
- New `unsafe` requires a short justification comment and must remain narrowly
  scoped.
- Do not hide missing functionality behind silent success, early exits, or
  vague fallback behavior.
- Unimplemented behavior must fail explicitly so tests can detect it.
- Do not treat `skip`, `not handled`, or `not reached` as `pass`.
- Keep comments sparse and in English. Explain only non-obvious logic.

## Testing Rules

- Tests are the primary quality gate. Human review is useful, but it is not
  the main proof of correctness.
- Every bug fix must add or update a regression test.
- Every new feature must include tests for the expected path, edge cases, and
  failure cases defined by the spec.
- Run the narrowest useful tests while iterating, then run the relevant full
  validation before finishing.
- A change is not done if tests fail, are silently skipped, or do not check the
  claimed behavior.
- Keep formatting clean and keep `cargo clippy -- -D warnings` passing for the
  touched code.

## Documentation Rules

- Update the spec whenever behavior, interfaces, or assumptions change.
- Update documentation when a change affects workflow, semantics, or
  verification.
- Keep this file focused on behavioral rules for future work.
- Do not turn this file into a project tour, directory listing, or code-level
  reference.

## Communication Rules

- State assumptions, risks, and verification results clearly.
- Ask focused questions when ambiguity would change behavior in a meaningful
  way.
- Surface conflicts between the request and the spec before implementing.
- When trade-offs exist, prefer the option that is easier to test and review.

## Review Exceptions

- Security-sensitive code still requires human review.
- Complex concurrency changes still require human review.
- `unsafe`-heavy design changes still require human review.
- In these cases, tests are necessary but not sufficient.

## Definition of Done

- The implementation matches the current spec.
- Relevant tests are present and passing.
- Failure modes are explicit and observable.
- New `unsafe` is justified and minimal.
- Documentation is updated where needed.

## Commit Rules

- Use English commit messages.
- Format the subject as `module: subject`.
- Keep the subject within 72 characters.
- Use the body for what changed and why, not a code walkthrough.
- For commits created in this repository, add a `Signed-off-by` line that
  matches the current `git config user.name` and `git config user.email`.
