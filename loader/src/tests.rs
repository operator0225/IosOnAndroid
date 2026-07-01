//! Unit tests against synthetic, hand-built Mach-O byte buffers.
//!
//! Every fixture here is constructed field-by-field in this file from the
//! public struct layouts in `format.rs` — no bytes from a real Apple binary
//! are used or needed (see `../docs/LEGAL.md`).

use crate::format::{
    CPU_TYPE_ARM64, LC_LOAD_DYLIB, LC_MAIN, LC_SEGMENT_64, LC_SYMTAB, LC_UNIXTHREAD, MH_EXECUTE,
    MH_MAGIC_64,
};
use crate::{parse, Entry, Export, Import, LoadError};

const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const MH_DYLIB: u32 = 0x6;
const ARM_THREAD_STATE64: u32 = 6;
const LC_DYSYMTAB: u32 = 0xb;

// nlist_64.n_type bits, for building test symbol tables.
const N_EXT: u8 = 0x01;
const N_UNDF: u8 = 0x00;
const N_SECT: u8 = 0x0e;
const N_STAB_MARKER: u8 = 0x20; // any bit within the 0xe0 N_STAB mask

struct SymtabPatch {
    /// Byte offset of the `symoff` field, relative to the start of `cmds`.
    symoff_field: usize,
    nlist_bytes: Vec<u8>,
    strtab_bytes: Vec<u8>,
}

struct Builder {
    cmds: Vec<u8>,
    ncmds: u32,
    cputype: u32,
    filetype: u32,
    symtab_patch: Option<SymtabPatch>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            cmds: Vec::new(),
            ncmds: 0,
            cputype: CPU_TYPE_ARM64,
            filetype: MH_EXECUTE,
            symtab_patch: None,
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

    fn dylib(mut self, name: &str) -> Self {
        let mut payload = Vec::new();
        payload.extend(24u32.to_le_bytes()); // name offset, relative to cmd_start
        payload.extend(0u32.to_le_bytes()); // timestamp
        payload.extend(0u32.to_le_bytes()); // current_version
        payload.extend(0u32.to_le_bytes()); // compatibility_version
        payload.extend(name.as_bytes());
        payload.push(0);
        while payload.len() % 8 != 0 {
            payload.push(0); // pad like a real dylib_command would
        }

        let mut cmd = Vec::new();
        cmd.extend(LC_LOAD_DYLIB.to_le_bytes());
        cmd.extend(((8 + payload.len()) as u32).to_le_bytes());
        cmd.extend(payload);
        self.cmds.extend(cmd);
        self.ncmds += 1;
        self
    }

    /// Each entry is `(name, n_type, n_value)`; string-table/nlist bytes
    /// are staged and only finalized (with real file offsets) in `build()`,
    /// since `LC_SYMTAB` points at a symbol table that lives *after* all
    /// load commands.
    fn symtab(mut self, syms: &[(&str, u8, u64)]) -> Self {
        let symoff_field = self.cmds.len() + 8; // past this cmd's cmd/cmdsize
        let mut cmd = Vec::new();
        cmd.extend(LC_SYMTAB.to_le_bytes());
        cmd.extend(24u32.to_le_bytes());
        cmd.extend(0u32.to_le_bytes()); // symoff, patched in build()
        cmd.extend((syms.len() as u32).to_le_bytes());
        cmd.extend(0u32.to_le_bytes()); // stroff, patched in build()
        cmd.extend(0u32.to_le_bytes()); // strsize, patched in build()
        self.cmds.extend(cmd);
        self.ncmds += 1;

        let mut strtab = vec![0u8]; // conventional leading NUL
        let mut nlist_bytes = Vec::new();
        for (name, n_type, n_value) in syms {
            let n_strx = strtab.len() as u32;
            strtab.extend(name.as_bytes());
            strtab.push(0);
            nlist_bytes.extend(n_strx.to_le_bytes());
            nlist_bytes.push(*n_type);
            nlist_bytes.push(0u8); // n_sect
            nlist_bytes.extend(0u16.to_le_bytes()); // n_desc
            nlist_bytes.extend(n_value.to_le_bytes());
        }

        self.symtab_patch = Some(SymtabPatch {
            symoff_field,
            nlist_bytes,
            strtab_bytes: strtab,
        });
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
        const HEADER_LEN: usize = 32;
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

        if let Some(patch) = self.symtab_patch {
            let symoff = out.len() as u32;
            let stroff = symoff + patch.nlist_bytes.len() as u32;
            let strsize = patch.strtab_bytes.len() as u32;

            let field_pos = HEADER_LEN + patch.symoff_field;
            out[field_pos..field_pos + 4].copy_from_slice(&symoff.to_le_bytes());
            out[field_pos + 8..field_pos + 12].copy_from_slice(&stroff.to_le_bytes());
            out[field_pos + 12..field_pos + 16].copy_from_slice(&strsize.to_le_bytes());

            out.extend(patch.nlist_bytes);
            out.extend(patch.strtab_bytes);
        }

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
    // LC_DYSYMTAB: a real command we deliberately don't interpret yet
    // (see docs/ROADMAP.md Phase 2) but must still skip correctly via its
    // own cmdsize.
    let bytes = Builder::new()
        .raw_cmd(LC_DYSYMTAB, 72)
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

#[test]
fn parses_load_dylib_names() {
    let bytes = Builder::new()
        .dylib("/usr/lib/libSystem.B.dylib")
        .dylib("/System/Library/Frameworks/Foundation.framework/Foundation")
        .lc_main(0, 0)
        .build();
    let image = parse(&bytes).expect("should parse");
    assert_eq!(
        image.dylibs,
        vec![
            "/usr/lib/libSystem.B.dylib".to_string(),
            "/System/Library/Frameworks/Foundation.framework/Foundation".to_string(),
        ]
    );
}

#[test]
fn parses_symtab_into_imports_and_exports() {
    let bytes = Builder::new()
        .symtab(&[
            ("_printf", N_UNDF | N_EXT, 0),      // imported (undefined external)
            ("_main", N_SECT | N_EXT, 0x1_0080), // exported (defined external)
        ])
        .lc_main(0x80, 0)
        .build();
    let image = parse(&bytes).expect("should parse");

    assert_eq!(
        image.imports,
        vec![Import {
            name: "_printf".to_string()
        }]
    );
    assert_eq!(
        image.exports,
        vec![Export {
            name: "_main".to_string(),
            value: 0x1_0080,
        }]
    );
}

#[test]
fn symtab_skips_local_and_debug_symbols() {
    let bytes = Builder::new()
        .symtab(&[
            ("_local_helper", N_SECT, 0x1_0000), // no N_EXT: local, not an import/export
            ("some_stab_entry", N_STAB_MARKER, 0), // debug symbol, not a real symbol
            ("_imported", N_UNDF | N_EXT, 0),
        ])
        .lc_main(0, 0)
        .build();
    let image = parse(&bytes).expect("should parse");
    assert_eq!(
        image.imports,
        vec![Import {
            name: "_imported".to_string()
        }]
    );
    assert!(image.exports.is_empty());
}

#[test]
fn no_symtab_command_yields_empty_import_export_lists() {
    let bytes = Builder::new().lc_main(0, 0).build();
    let image = parse(&bytes).expect("should parse");
    assert!(image.imports.is_empty());
    assert!(image.exports.is_empty());
}

#[test]
fn rejects_duplicate_symtab() {
    let bytes = Builder::new()
        .symtab(&[("_a", N_UNDF | N_EXT, 0)])
        .symtab(&[("_b", N_UNDF | N_EXT, 0)])
        .lc_main(0, 0)
        .build();
    assert_eq!(parse(&bytes), Err(LoadError::DuplicateSymtab));
}
