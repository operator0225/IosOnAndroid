//! Builds a minimal, valid ARM64 Mach-O64 executable for local testing of
//! the `loader` and `syscall-shim` crates.
//!
//! The machine code is original and hand-assembled (see `asm/hello.s`),
//! not extracted from any Apple binary — see `../../docs/LEGAL.md`.

/// Raw ARM64 instruction bytes for `asm/hello.s`: `write(1, "Hi\n", 3)`
/// then `exit(0)`, using raw Darwin BSD syscalls (`svc #0x80`) directly —
/// no libSystem, no dynamic linking, nothing beyond what Phase 1's
/// syscall-shim understands. Verified with `llvm-mc`/`llvm-objdump`; see
/// that file for the exact regeneration command and a disassembly-level
/// sanity check.
const HELLO_TEXT: &[u8] = &[
    0x20, 0x00, 0x80, 0xd2, // mov x0, #1
    0xe1, 0x00, 0x00, 0x10, // adr x1, msg
    0x62, 0x00, 0x80, 0xd2, // mov x2, #3
    0x90, 0x00, 0x80, 0xd2, // mov x16, #4
    0x01, 0x10, 0x00, 0xd4, // svc #0x80
    0x00, 0x00, 0x80, 0xd2, // mov x0, #0
    0x30, 0x00, 0x80, 0xd2, // mov x16, #1
    0x01, 0x10, 0x00, 0xd4, // svc #0x80
    0x48, 0x69, 0x0a, // "Hi\n"
];

const PAGE_SIZE: u64 = 0x4000; // iOS's arm64 page size (16K)
const TEXT_VMADDR: u64 = 0x1_0000;

const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const MH_EXECUTE: u32 = 0x2;
const LC_SEGMENT_64: u32 = 0x19;
const LC_MAIN: u32 = 0x28 | 0x8000_0000;
const VM_PROT_READ: u32 = 0x1;
const VM_PROT_EXECUTE: u32 = 0x4;

/// Builds the fixture and returns it along with the entry offset used, so
/// callers/tests don't have to recompute the header layout independently.
pub fn build_hello() -> Vec<u8> {
    const HEADER_LEN: usize = 32;
    const SEGMENT_CMD_LEN: usize = 72;
    const MAIN_CMD_LEN: usize = 24;
    let entryoff = (HEADER_LEN + SEGMENT_CMD_LEN + MAIN_CMD_LEN) as u64;
    let file_len = entryoff + HELLO_TEXT.len() as u64;
    let vmsize = file_len.div_ceil(PAGE_SIZE) * PAGE_SIZE;

    let mut out = Vec::with_capacity(file_len as usize);

    // mach_header_64
    out.extend(MH_MAGIC_64.to_le_bytes());
    out.extend(CPU_TYPE_ARM64.to_le_bytes());
    out.extend(0u32.to_le_bytes()); // cpusubtype
    out.extend(MH_EXECUTE.to_le_bytes());
    out.extend(2u32.to_le_bytes()); // ncmds
    out.extend(((SEGMENT_CMD_LEN + MAIN_CMD_LEN) as u32).to_le_bytes()); // sizeofcmds
    out.extend(0u32.to_le_bytes()); // flags
    out.extend(0u32.to_le_bytes()); // reserved
    assert_eq!(out.len(), HEADER_LEN);

    // LC_SEGMENT_64 __TEXT, covering the whole file (headers + code)
    out.extend(LC_SEGMENT_64.to_le_bytes());
    out.extend((SEGMENT_CMD_LEN as u32).to_le_bytes());
    let mut segname = [0u8; 16];
    segname[..6].copy_from_slice(b"__TEXT");
    out.extend(segname);
    out.extend(TEXT_VMADDR.to_le_bytes());
    out.extend(vmsize.to_le_bytes());
    out.extend(0u64.to_le_bytes()); // fileoff
    out.extend(file_len.to_le_bytes()); // filesize
    out.extend((VM_PROT_READ | VM_PROT_EXECUTE).to_le_bytes()); // maxprot
    out.extend((VM_PROT_READ | VM_PROT_EXECUTE).to_le_bytes()); // initprot
    out.extend(0u32.to_le_bytes()); // nsects
    out.extend(0u32.to_le_bytes()); // flags
    assert_eq!(out.len(), HEADER_LEN + SEGMENT_CMD_LEN);

    // LC_MAIN
    out.extend(LC_MAIN.to_le_bytes());
    out.extend((MAIN_CMD_LEN as u32).to_le_bytes());
    out.extend(entryoff.to_le_bytes());
    out.extend(0x1000u64.to_le_bytes()); // stacksize
    assert_eq!(out.len(), entryoff as usize);

    out.extend_from_slice(HELLO_TEXT);
    assert_eq!(out.len() as u64, file_len);

    out
}

/// Expected runtime entry address for [`build_hello`]'s output, i.e.
/// `TEXT_VMADDR + entryoff`, for tests/callers that want to check it
/// without re-deriving the header layout.
pub fn expected_entry_vmaddr() -> u64 {
    const HEADER_LEN: u64 = 32;
    const SEGMENT_CMD_LEN: u64 = 72;
    const MAIN_CMD_LEN: u64 = 24;
    TEXT_VMADDR + HEADER_LEN + SEGMENT_CMD_LEN + MAIN_CMD_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_loader() {
        let bytes = build_hello();
        let image = loader::parse(&bytes).expect("fixture should be valid Mach-O");

        assert_eq!(image.segments.len(), 1);
        let text = &image.segments[0];
        assert_eq!(text.name, "__TEXT");
        assert_eq!(text.vmaddr, TEXT_VMADDR);
        assert!(text.initprot.read && text.initprot.execute && !text.initprot.write);

        assert_eq!(image.entry_vmaddr().unwrap(), expected_entry_vmaddr());

        // The bytes at the entry offset are exactly our hand-assembled
        // code, not header/load-command bytes.
        let entry_file_off =
            text.fileoff as usize + (image.entry_vmaddr().unwrap() - text.vmaddr) as usize;
        assert_eq!(
            &bytes[entry_file_off..entry_file_off + HELLO_TEXT.len()],
            HELLO_TEXT
        );
    }
}
