//! Bounds-checked little-endian byte reader.
//!
//! Mach-O input is untrusted (it's the whole point of this crate), so every
//! read here is fallible instead of panicking on short/malformed input.

#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OutOfBounds;

impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], OutOfBounds> {
        let end = self.pos.checked_add(n).ok_or(OutOfBounds)?;
        let slice = self.bytes.get(self.pos..end).ok_or(OutOfBounds)?;
        self.pos = end;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8, OutOfBounds> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, OutOfBounds> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, OutOfBounds> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, OutOfBounds> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], OutOfBounds> {
        self.take(n)
    }

    pub fn seek(&mut self, pos: usize) -> Result<(), OutOfBounds> {
        if pos > self.bytes.len() {
            return Err(OutOfBounds);
        }
        self.pos = pos;
        Ok(())
    }

    /// Reads a NUL-terminated byte string starting at `start`, without
    /// disturbing this cursor's own position. Errors (rather than reading
    /// past `limit`/end-of-buffer) if no NUL terminator is found in range.
    pub fn cstr_bytes_at(&self, start: usize, limit: usize) -> Result<&'a [u8], OutOfBounds> {
        if start > limit || limit > self.bytes.len() {
            return Err(OutOfBounds);
        }
        let window = &self.bytes[start..limit];
        let nul = window.iter().position(|&b| b == 0).ok_or(OutOfBounds)?;
        Ok(&window[..nul])
    }
}
