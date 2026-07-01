//! ARM64 Mach-O binary parsing.
//!
//! Parses just enough of the Mach-O64 container format (public file-format
//! knowledge, no Apple code involved — see `../docs/LEGAL.md`) to describe
//! how a binary's segments should be mapped into memory and where execution
//! should start. This crate does no mapping/execution itself; see
//! `syscall-shim` and the future `runtime-shim` for what runs a parsed
//! image.
//!
//! Scope for Phase 1 (see `../docs/ROADMAP.md`): `mach_header_64`,
//! `LC_SEGMENT_64`, `LC_MAIN`, and `LC_UNIXTHREAD` (ARM64 thread state only).
//! Dynamic-library load commands are recorded but not resolved yet — that's
//! Phase 2.

mod cursor;
mod format;
mod parse;

pub use format::{
    Entry, Export, Import, Prot, Segment, CPU_TYPE_ARM64, MH_EXECUTE, MH_MAGIC_64, VM_PROT_EXECUTE,
    VM_PROT_READ, VM_PROT_WRITE,
};
pub use parse::{parse, LoadError, MachOImage};

#[cfg(test)]
mod tests;
