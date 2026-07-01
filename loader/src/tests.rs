//! Unit tests against synthetic, hand-built Mach-O byte buffers.
//!
//! Every fixture here is constructed field-by-field in this file from the
//! public struct layouts in `format.rs` — no bytes from a real Apple binary
//! are used or needed (see `../docs/LEGAL.md`).

use crate::format::{
    CPU_TYPE_ARM64, LC_MAIN, LC_SEGMENT_64, LC_UNIXTHREAD, MH_EXECUTE, MH_MAGIC_64,
};
use crate::{parse, Entry, LoadError};

const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const MH_DYLIB: u32 = 0x6;
const ARM_THREAD_STATE64: u32 = 6;

struct Builder {
    cmds: Vec<u8>,
    ncmds: u32,
    cputype: u32,
    filetype: u32,
}

impl Builder {
    fn new() -> Self {
        Builder {
            cmds: Vec::new(),
            ncmds: 0,
            cputype: CPU_TYPE_ARM64,
            filetype: MH_EXECUTE,
        }
    }

    fn cputype(mut self, v: u32) -> Self {
        self.cputype = v;
        self
    }

    fn filetype(mut self, v: u32) -> Self {
        self.filetype = v;
        self
    }

    fn segment(mut self, name: &str, vmaddr: u64, vmsize: u64, prot: u32) -> Self {
        let mut segname = [0u8; 16];
        segname[..name.len()].copy_from_slice(name.as_bytes());

        let mut cmd = Vec::new();
        cmd.extend(LC_SEGMENT_64.to_le_bytes());
        // cmdsize patched below
        cmd.extend(0u32.to_le_bytes());
        cmd.extend(segname);
        cmd.extend(vmaddr.to_le_bytes());
        cmd.extend(vmsize.to_le_bytes());
        cmd.extend(vmaddr.to_le_bytes()); // fileoff == vmaddr for these fixtures
        cmd.extend(vmsize.to_le_bytes()); // filesize
        cmd.extend(prot.to_le_bytes()); // maxprot
        cmd.extend(prot.to_le_bytes()); // initprot
        cmd.extend(0u32.to_le_bytes()); // nsects
        cmd.extend(0u32.to_le_bytes()); // flags
        let cmdsize = cmd.len() as u32;
        cmd[4..8].copy_from_slice(&cmdsize.to_le_bytes());

        self.cmds.extend(cmd);
        self.ncmds += 1;
        self
    }

    fn lc_main(mut self, entryoff: u64, stacksize: u64) -> Self {
        let mut cmd = Vec::new();
        cmd.extend(LC_MAIN.to_le_bytes());
        cmd.extend(24u32.to_le_bytes());
        cmd.extend(entryoff.to_le_bytes());
        cmd.extend(stacksize.to_le_bytes());
        self.cmds.extend(cmd);
        self.ncmds += 1;
        self
    }

    /// Classic (non-pointer-auth) `ARM_THREAD_STATE64`: x0..x28 (29 regs),
    /// fp, lr, sp, pc = 33 8-byte words.
    fn lc_unixthread(mut self, pc: u64) -> Self {
        let mut state = Vec::new();
        for _ in 0..32 {
            state.extend(0u64.to_le_bytes()); // x0..x28, fp, lr, sp
        }
        state.extend(pc.to_le_bytes());
        assert_eq!(state.len(), 33 * 8);

        let mut cmd = Vec::new();
        cmd.extend(LC_UNIXTHREAD.to_le_bytes());
        let cmdsize = 8 + 8 + state.len() as u32;
        cmd.extend(cmdsize.to_le_bytes());
        cmd.extend(ARM_THREAD_STATE64.to_le_bytes());
        cmd.extend(((state.len() / 4) as u32).to_le_bytes());
        cmd.extend(state);
        self.cmds.extend(cmd);
        self.ncmds += 1;
        self
    }

    fn raw_cmd(mut self, cmd: u32, extra_payload_len: usize) -> Self {
        let cmdsize = 8 + extra_payload_len as u32;
        let mut c = Vec::new();
        c.extend(cmd.to_le_bytes());
        c.extend(cmdsize.to_le_bytes());
        c.extend(vec![0u8; extra_payload_len]);
        self.cmds.extend(c);
        self.ncmds += 1;
        self
    }

    fn build(self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(MH_MAGIC_64.to_le_bytes());
        out.extend(self.cputype.to_le_bytes());
        out.extend(0u32.to_le_bytes()); // cpusubtype
        out.extend(self.filetype.to_le_bytes());
        out.extend(self.ncmds.to_le_bytes());
        out.extend((self.cmds.len() as u32).to_le_bytes()); // sizeofcmds
        out.extend(0u32.to_le_bytes()); // flags
        out.extend(0u32.to_le_bytes()); // reserved
        out.extend(self.cmds);
        out
    }
}

#[test]
fn parses_lc_main_binary() {
    let bytes = Builder::new()
        .segment("__TEXT", 0x1_0000, 0x4000, 0x5) // r-x
        .segment("__DATA", 0x1_4000, 0x1000, 0x3) // rw-
        .lc_main(0x100, 0)
        .build();

    let image = parse(&bytes).expect("should parse");
    assert_eq!(image.segments.len(), 2);
    assert_eq!(image.segments[0].name, "__TEXT");
    assert_eq!(image.segments[0].vmaddr, 0x1_0000);
    assert!(image.segments[0].initprot.read);
    assert!(!image.segments[0].initprot.write);
    assert!(image.segments[0].initprot.execute);
    assert_eq!(image.segments[1].name, "__DATA");
    assert!(image.segments[1].initprot.write);
    assert!(!image.segments[1].initprot.execute);

    assert_eq!(image.entry, Entry::TextOffset(0x100));
    assert_eq!(image.entry_vmaddr().unwrap(), 0x1_0000 + 0x100);
}

#[test]
fn parses_lc_unixthread_binary() {
    let bytes = Builder::new()
        .segment("__TEXT", 0x1_0000, 0x4000, 0x5)
        .lc_unixthread(0x1_0200)
        .build();

    let image = parse(&bytes).expect("should parse");
    assert_eq!(image.entry, Entry::AbsolutePc(0x1_0200));
    assert_eq!(image.entry_vmaddr().unwrap(), 0x1_0200);
}

#[test]
fn segname_without_trailing_nul_is_handled() {
    // "__DATA_CONST" padded to exactly 16 bytes with no NUL terminator at
    // all is a valid (if unusual) segname; make sure we don't panic on it.
    let bytes = Builder::new()
        .segment("1234567890123456", 0x2000, 0x1000, 0x1)
        .lc_main(0, 0)
        .build();
    let image = parse(&bytes).expect("should parse");
    assert_eq!(image.segments[0].name, "1234567890123456");
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = Builder::new().lc_main(0, 0).build();
    bytes[0] = 0xff;
    let expected_magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    assert_eq!(parse(&bytes), Err(LoadError::BadMagic(expected_magic)));
}

#[test]
fn rejects_non_arm64_cputype() {
    let bytes = Builder::new()
        .cputype(CPU_TYPE_X86_64)
        .lc_main(0, 0)
        .build();
    assert_eq!(
        parse(&bytes),
        Err(LoadError::UnsupportedCpuType(CPU_TYPE_X86_64))
    );
}

#[test]
fn rejects_non_executable_filetype() {
    let bytes = Builder::new().filetype(MH_DYLIB).lc_main(0, 0).build();
    assert_eq!(parse(&bytes), Err(LoadError::UnsupportedFileType(MH_DYLIB)));
}

#[test]
fn rejects_truncated_input() {
    let full = Builder::new()
        .segment("__TEXT", 0x1_0000, 0x4000, 0x5)
        .lc_main(0, 0)
        .build();
    let truncated = &full[..full.len() - 4];
    assert_eq!(parse(truncated), Err(LoadError::Truncated));
}

#[test]
fn rejects_cmdsize_running_past_sizeofcmds() {
    let mut bytes = Builder::new().lc_main(0, 0).build();
    // Corrupt LC_MAIN's cmdsize (bytes 32..36, right after the 32-byte
    // header) to claim it's larger than the space sizeofcmds reserved.
    bytes[32 + 4..32 + 8].copy_from_slice(&999u32.to_le_bytes());
    match parse(&bytes) {
        Err(LoadError::BadCmdSize { .. }) => {}
        other => panic!("expected BadCmdSize, got {other:?}"),
    }
}

#[test]
fn rejects_missing_entry_point() {
    let bytes = Builder::new()
        .segment("__TEXT", 0x1_0000, 0x4000, 0x5)
        .build();
    assert_eq!(parse(&bytes), Err(LoadError::MissingEntryPoint));
}

#[test]
fn rejects_ambiguous_dual_entry_point() {
    let bytes = Builder::new()
        .lc_main(0x100, 0)
        .lc_unixthread(0x1_0200)
        .build();
    assert_eq!(parse(&bytes), Err(LoadError::DuplicateEntryPoint));
}

#[test]
fn unknown_load_commands_are_skipped() {
    // e.g. a stand-in for LC_LOAD_DYLIB, whose contents we don't interpret
    // in Phase 1 (see docs/ROADMAP.md Phase 2) but must skip correctly.
    const LC_LOAD_DYLIB_STANDIN: u32 = 0xC; // arbitrary unhandled cmd value
    let bytes = Builder::new()
        .raw_cmd(LC_LOAD_DYLIB_STANDIN, 40)
        .segment("__TEXT", 0x1_0000, 0x4000, 0x5)
        .lc_main(0x10, 0)
        .build();
    let image = parse(&bytes).expect("should parse past unknown command");
    assert_eq!(image.segments.len(), 1);
    assert_eq!(image.entry, Entry::TextOffset(0x10));
}

#[test]
fn entry_vmaddr_without_text_segment_errors() {
    let bytes = Builder::new().lc_main(0x10, 0).build();
    let image = parse(&bytes).expect("should parse");
    assert_eq!(image.entry_vmaddr(), Err(LoadError::MissingTextSegment));
}
