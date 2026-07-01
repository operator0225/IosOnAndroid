//! Mach-O64 constants and the parsed-output types.
//!
//! Values below are from the public Mach-O file format (documented by
//! Apple's own open-source `<mach-o/loader.h>` / `<mach/machine.h>`
//! headers, part of the APSL-licensed `xnu` project). These are file-format
//! constants, not implementation code — the same category of public
//! knowledge as an ELF or PE magic number.

/// `mach_header_64.magic` for a 64-bit little-endian Mach-O.
pub const MH_MAGIC_64: u32 = 0xfeed_facf;

/// `mach_header_64.cputype` for ARM64/AArch64.
pub const CPU_TYPE_ARM64: u32 = 0x0100_000c;

/// `mach_header_64.filetype` for a standalone executable.
pub const MH_EXECUTE: u32 = 0x2;

pub(crate) const LC_REQ_DYLD: u32 = 0x8000_0000;
pub(crate) const LC_SEGMENT_64: u32 = 0x19;
pub(crate) const LC_UNIXTHREAD: u32 = 0x5;
pub(crate) const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;

/// Thread-state flavor for `LC_UNIXTHREAD` on ARM64 (`ARM_THREAD_STATE64`).
pub(crate) const ARM_THREAD_STATE64: u32 = 6;

/// `vm_prot_t` bit for read access.
pub const VM_PROT_READ: u32 = 0x1;
/// `vm_prot_t` bit for write access.
pub const VM_PROT_WRITE: u32 = 0x2;
/// `vm_prot_t` bit for execute access.
pub const VM_PROT_EXECUTE: u32 = 0x4;

/// Memory protection for a segment, decoded from `vm_prot_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Prot {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl From<u32> for Prot {
    fn from(bits: u32) -> Self {
        Prot {
            read: bits & VM_PROT_READ != 0,
            write: bits & VM_PROT_WRITE != 0,
            execute: bits & VM_PROT_EXECUTE != 0,
        }
    }
}

/// One `LC_SEGMENT_64` load command, describing a range of the file that
/// should be mapped into the process's address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Segment name, e.g. `__TEXT`, `__DATA` (NUL-trimmed).
    pub name: String,
    pub vmaddr: u64,
    pub vmsize: u64,
    pub fileoff: u64,
    pub filesize: u64,
    pub maxprot: Prot,
    pub initprot: Prot,
}

/// Where execution should begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// From `LC_MAIN`: offset from the `__TEXT` segment's file offset.
    /// The loader adds this to `__TEXT`'s `vmaddr` (after any ASLR slide)
    /// to get the runtime entry address.
    TextOffset(u64),
    /// From `LC_UNIXTHREAD`: an absolute initial program-counter value
    /// taken directly from the embedded ARM64 thread state.
    AbsolutePc(u64),
}
