# tools

Build scripts, local test-fixture generation helpers, and the automated
test harness.

- `mkfixture/` — builds a minimal, valid ARM64 Mach-O64 executable from
  original, hand-assembled machine code (see `mkfixture/asm/hello.s`,
  assembled with `llvm-mc`, not any Apple toolchain — see
  [../docs/LEGAL.md](../docs/LEGAL.md)). Used to test `loader`/
  `syscall-shim` without needing a real iOS binary. Run with `cargo run -p
  iosonandroid-mkfixture -- <output-path>`; output is gitignored.

Real iOS app binaries (for testing framework/app compatibility in later
phases) are a different thing: those must be built locally by a developer
with their own Apple toolchain and never committed to this repo.

Status: Phase 1, `mkfixture` done. See
[Phase 0](../docs/ROADMAP.md#phase-0--architecture--scaffolding-current)
and [Phase 7](../docs/ROADMAP.md#phase-7--breadth-testing-hardening).
