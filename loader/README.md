# loader

ARM64 Mach-O binary parser (`cargo test -p iosonandroid-loader`).

Parses the Mach-O64 header, `LC_SEGMENT_64`, `LC_MAIN`, and `LC_UNIXTHREAD`
(ARM64 thread state) into a `MachOImage` describing what to map and where
to start execution. Bounds-checked against malformed input (fuzzable
territory — this parses untrusted binaries by design).

Not yet implemented: actually mapping segments into a live process (that's
where this crate's output gets consumed, by the not-yet-written runner —
see Phase 1 in the roadmap), dynamic-library load commands, and
`__LINKEDIT`/symbol table parsing (Phase 2).

Status: Phase 1, parser done + unit tested. See
[Phase 1](../docs/ROADMAP.md#phase-1--mach-o-loader--hello-syscall) and
[Architecture §1](../docs/ARCHITECTURE.md#1-loader-loader).
