use crate::cursor::{Cursor, OutOfBounds};
use crate::format::{
    Entry, Prot, Segment, ARM_THREAD_STATE64, CPU_TYPE_ARM64, LC_MAIN, LC_SEGMENT_64,
    LC_UNIXTHREAD, MH_EXECUTE, MH_MAGIC_64,
};

/// A successfully parsed Mach-O64 image: everything a loader needs to know
/// to map segments and pick a start address, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachOImage {
    pub cputype: u32,
    pub filetype: u32,
    pub segments: Vec<Segment>,
    pub entry: Entry,
}

impl MachOImage {
    pub fn text_segment(&self) -> Option<&Segment> {
        self.segments.iter().find(|s| s.name == "__TEXT")
    }

    /// Absolute entry address in the file's own (unslid) address space.
    /// Applying an ASLR slide is the runtime loader's job, not this crate's.
    pub fn entry_vmaddr(&self) -> Result<u64, LoadError> {
        match self.entry {
            Entry::AbsolutePc(pc) => Ok(pc),
            Entry::TextOffset(off) => {
                let text = self.text_segment().ok_or(LoadError::MissingTextSegment)?;
                text.vmaddr.checked_add(off).ok_or(LoadError::Malformed)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Truncated,
    BadMagic(u32),
    UnsupportedCpuType(u32),
    UnsupportedFileType(u32),
    BadCmdSize { cmd: u32, cmdsize: u32 },
    BadSegname,
    DuplicateEntryPoint,
    MissingEntryPoint,
    MissingTextSegment,
    UnsupportedThreadFlavor(u32),
    Malformed,
}

impl From<OutOfBounds> for LoadError {
    fn from(_: OutOfBounds) -> Self {
        LoadError::Truncated
    }
}

/// Parse a Mach-O64 ARM64 executable from `bytes`.
///
/// Only understands what Phase 1 needs: the header, `LC_SEGMENT_64`,
/// `LC_MAIN`, and `LC_UNIXTHREAD` (ARM64 flavor). Anything else
/// (`LC_LOAD_DYLIB`, symbol tables, code signatures, ...) is skipped via
/// each load command's own `cmdsize`, not interpreted — resolving those is
/// later-phase work (see `../docs/ROADMAP.md`).
pub fn parse(bytes: &[u8]) -> Result<MachOImage, LoadError> {
    let mut c = Cursor::new(bytes);

    let magic = c.u32()?;
    if magic != MH_MAGIC_64 {
        return Err(LoadError::BadMagic(magic));
    }
    let cputype = c.u32()?;
    if cputype != CPU_TYPE_ARM64 {
        return Err(LoadError::UnsupportedCpuType(cputype));
    }
    let _cpusubtype = c.u32()?;
    let filetype = c.u32()?;
    if filetype != MH_EXECUTE {
        return Err(LoadError::UnsupportedFileType(filetype));
    }
    let ncmds = c.u32()?;
    let sizeofcmds = c.u32()?;
    let _flags = c.u32()?;
    let _reserved = c.u32()?;

    let cmds_start = c.pos();
    let cmds_end = cmds_start
        .checked_add(sizeofcmds as usize)
        .ok_or(LoadError::Malformed)?;
    if cmds_end > bytes.len() {
        return Err(LoadError::Truncated);
    }

    let mut segments = Vec::new();
    let mut entry: Option<Entry> = None;

    for _ in 0..ncmds {
        let cmd_start = c.pos();
        if cmd_start >= cmds_end {
            return Err(LoadError::Malformed);
        }
        let cmd = c.u32()?;
        let cmdsize = c.u32()?;
        if cmdsize < 8 {
            return Err(LoadError::BadCmdSize { cmd, cmdsize });
        }
        let next_cmd = cmd_start
            .checked_add(cmdsize as usize)
            .ok_or(LoadError::Malformed)?;
        if next_cmd > cmds_end {
            return Err(LoadError::BadCmdSize { cmd, cmdsize });
        }

        match cmd {
            LC_SEGMENT_64 => segments.push(parse_segment(&mut c)?),
            LC_MAIN => {
                let entryoff = c.u64()?;
                let _stacksize = c.u64()?;
                set_entry(&mut entry, Entry::TextOffset(entryoff))?;
            }
            LC_UNIXTHREAD => {
                if let Some(pc) = parse_unixthread(&mut c, cmd_start, next_cmd)? {
                    set_entry(&mut entry, Entry::AbsolutePc(pc))?;
                }
            }
            _ => {}
        }

        c.seek(next_cmd)?;
    }

    Ok(MachOImage {
        cputype,
        filetype,
        segments,
        entry: entry.ok_or(LoadError::MissingEntryPoint)?,
    })
}

fn set_entry(slot: &mut Option<Entry>, value: Entry) -> Result<(), LoadError> {
    if slot.is_some() {
        return Err(LoadError::DuplicateEntryPoint);
    }
    *slot = Some(value);
    Ok(())
}

fn parse_segment(c: &mut Cursor) -> Result<Segment, LoadError> {
    let segname_raw = c.bytes(16)?;
    let nul = segname_raw
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(segname_raw.len());
    let name = std::str::from_utf8(&segname_raw[..nul])
        .map_err(|_| LoadError::BadSegname)?
        .to_string();

    let vmaddr = c.u64()?;
    let vmsize = c.u64()?;
    let fileoff = c.u64()?;
    let filesize = c.u64()?;
    let maxprot = Prot::from(c.u32()?);
    let initprot = Prot::from(c.u32()?);
    let _nsects = c.u32()?;
    let _flags = c.u32()?;

    Ok(Segment {
        name,
        vmaddr,
        vmsize,
        fileoff,
        filesize,
        maxprot,
        initprot,
    })
}

/// Extracts the initial PC from an `LC_UNIXTHREAD`'s `ARM_THREAD_STATE64`
/// blob. Only the classic (non-pointer-authentication) 33-register layout
/// is understood; newer variants with an appended `flags`/`pad` field for
/// pointer auth are reported as [`LoadError::UnsupportedThreadFlavor`]
/// rather than silently misparsed.
fn parse_unixthread(
    c: &mut Cursor,
    cmd_start: usize,
    next_cmd: usize,
) -> Result<Option<u64>, LoadError> {
    let flavor = c.u32()?;
    let count = c.u32()?;
    let state_len = (count as usize)
        .checked_mul(4)
        .ok_or(LoadError::Malformed)?;
    let state_start = c.pos();
    let state_end = state_start
        .checked_add(state_len)
        .ok_or(LoadError::Malformed)?;
    if state_end > next_cmd || cmd_start >= state_start {
        return Err(LoadError::Malformed);
    }

    if flavor != ARM_THREAD_STATE64 {
        return Ok(None);
    }

    // arm_thread_state64_t (classic layout): x[0..29], fp, lr, sp, pc = 33
    // 8-byte words, pc is the last one.
    const CLASSIC_WORDS: usize = 33;
    if state_len < CLASSIC_WORDS * 8 {
        return Err(LoadError::UnsupportedThreadFlavor(flavor));
    }
    let pc_offset = (CLASSIC_WORDS - 1) * 8;
    let save = c.pos();
    c.seek(state_start + pc_offset)?;
    let pc = c.u64()?;
    c.seek(save)?;
    Ok(Some(pc))
}
