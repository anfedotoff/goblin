//! "Nlist" style symbols in this binary - beware, like most symbol tables in most binary formats, they are strippable, and should not be relied upon, see the imports and exports modules for something more permanent.
//!
//! Symbols are essentially a type, offset, and the symbol name

use scroll::{self, ctx, Pread};
use scroll::ctx::SizeWith;
use error;
use container::{self, Container};
use mach::load_command;
use core::fmt::{self, Debug};

pub const NLIST_TYPE_MASK: u8 = 0xe;
pub const NLIST_TYPE_GLOBAL: u8 = 0x1;
pub const NLIST_TYPE_LOCAL: u8 = 0x0;

#[repr(C)]
#[derive(Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Nlist32 {
    /// index into the string table
    n_strx: u32,
    /// type flag, see below
    n_type: u8,
    /// section number or NO_SECT
    n_sect: u8,
    /// see <mach-o/stab.h>
    n_desc: u16,
    /// value of this symbol (or stab offset)
    n_value: u32,
}

pub const SIZEOF_NLIST_32: usize = 12;

impl Debug for Nlist32 {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "strx: {:04} type: {:#02x} sect: {:#x} desc: {:#03x} value: {:#x}",
               self.n_strx,
               self.n_type,
               self.n_sect,
               self.n_desc,
               self.n_value,
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Nlist64 {
    /// index into the string table
    n_strx: u32,
    /// type flag, see below
    n_type: u8,
    /// section number or NO_SECT
    n_sect: u8,
    /// see <mach-o/stab.h>
    n_desc: u16,
    /// value of this symbol (or stab offset)
    n_value: u64,
}

pub const SIZEOF_NLIST_64: usize = 16;

impl Debug for Nlist64 {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "strx: {:04} type: {:#02x} sect: {:#x} desc: {:#03x} value: {:#x}",
               self.n_strx,
               self.n_type,
               self.n_sect,
               self.n_desc,
               self.n_value,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Nlist {
    /// index into the string table
    n_strx: usize,
    /// type flag, see below
    n_type: u8,
    /// section number or NO_SECT
    n_sect: usize,
    /// see <mach-o/stab.h>
    n_desc: u16,
    /// value of this symbol (or stab offset)
    n_value: u64,
}

impl ctx::SizeWith<container::Ctx> for Nlist {
    type Units = usize;
    fn size_with(ctx: &container::Ctx) -> usize {
        use container::Container;
        match ctx.container {
            Container::Little => {
                SIZEOF_NLIST_32
            },
            Container::Big => {
                SIZEOF_NLIST_64
            },
        }
    }
}

impl From<Nlist32> for Nlist {
    fn from(nlist: Nlist32) -> Self {
        Nlist {
            n_strx: nlist.n_strx as usize,
            n_type: nlist.n_type,
            n_sect: nlist.n_sect as usize,
            n_desc: nlist.n_desc,
            n_value: nlist.n_value as u64,
        }
    }
}

impl From<Nlist64> for Nlist {
    fn from(nlist: Nlist64) -> Self {
        Nlist {
            n_strx: nlist.n_strx as usize,
            n_type: nlist.n_type,
            n_sect: nlist.n_sect as usize,
            n_desc: nlist.n_desc,
            n_value: nlist.n_value,
        }
    }
}

impl<'a> ctx::TryFromCtx<'a, container::Ctx> for Nlist {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], container::Ctx { container, le }: container::Ctx) -> scroll::Result<(Self, Self::Size)> {
        let nlist = match container {
            Container::Little => {
                (bytes.pread_with::<Nlist32>(0, le)?.into(), SIZEOF_NLIST_32)
            },
            Container::Big => {
                (bytes.pread_with::<Nlist64>(0, le)?.into(), SIZEOF_NLIST_64)
            },
        };
        Ok(nlist)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SymbolsCtx {
    pub nsyms: usize,
    pub strtab: usize,
    pub ctx: container::Ctx,
}

impl<'a, T: ?Sized> ctx::TryFromCtx<'a, SymbolsCtx, T> for Symbols<'a> where T: AsRef<[u8]> {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a T, SymbolsCtx {
        nsyms, strtab, ctx
    }: SymbolsCtx) -> scroll::Result<(Self, Self::Size)> {
        let data = bytes.as_ref();
        Ok ((Symbols {
            data: data,
            start: 0,
            nsyms: nsyms,
            strtab: strtab,
            ctx: ctx,
        }, data.len()))
    }
}

pub struct SymbolIterator<'a> {
    data: &'a [u8],
    nsyms: usize,
    offset: usize,
    count: usize,
    ctx: container::Ctx,
    strtab: usize,
}

impl<'a> Iterator for SymbolIterator<'a> {
    type Item = error::Result<(&'a str, Nlist)>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.nsyms {
            None
        } else {
            self.count += 1;
            match self.data.gread_with::<Nlist>(&mut self.offset, self.ctx) {
                Ok(symbol) => {
                    match self.data.pread(self.strtab + symbol.n_strx) {
                        Ok(name) => {
                            Some(Ok((name, symbol)))
                        },
                        Err(e) => return Some(Err(e.into()))
                    }
                },
                Err(e) => return Some(Err(e.into()))
            }
        }
    }
}

/// A zero-copy "nlist" style symbol table ("stab"), including the string table
pub struct Symbols<'a> {
    data: &'a [u8],
    start: usize,
    nsyms: usize,
    // TODO: we can use an actual strtab here and tie it to symbols lifetime
    strtab: usize,
    ctx: container::Ctx,
}

impl<'a> Symbols<'a> {
    /// Creates a new symbol table with `count` elements, from the `start` offset, using the string table at `strtab`, with a _default_ ctx.
    ////
    /// **Beware**, this will provide incorrect results if you construct this on a 32-bit mach binary, using a 64-bit machine; use `parse` instead if you want 32/64 bit support
    pub fn new(bytes: &'a [u8], start: usize, count: usize, strtab: usize) -> error::Result<Symbols<'a>> {
        let nsyms = count;
        Ok (Symbols {
            data: bytes,
            start: start,
            nsyms: nsyms,
            strtab: strtab,
            ctx: container::Ctx::default(),
        })
    }
    pub fn parse(bytes: &'a [u8], symtab: &load_command::SymtabCommand, ctx: container::Ctx) -> error::Result<Symbols<'a>> {
        // we need to normalize the strtab offset before we receive the truncated bytes in pread_with
        let strtab = symtab.stroff - symtab.symoff;
        Ok(bytes.pread_with(symtab.symoff as usize, SymbolsCtx { nsyms: symtab.nsyms as usize, strtab: strtab as usize, ctx: ctx })?)
    }

    pub fn iter(&self) -> SymbolIterator {
        SymbolIterator {
            offset: self.start as usize,
            nsyms: self.nsyms as usize,
            count: 0,
            data: self.data,
            ctx: self.ctx,
            strtab: self.strtab,
        }
    }

    /// Parses a single Nlist symbol from the binary, with its accompanying name
    pub fn get(&self, index: usize) -> scroll::Result<(&'a str, Nlist)> {
        let sym: Nlist = self.data.pread_with(self.start + (index * Nlist::size_with(&self.ctx)), self.ctx)?;
        let name = self.data.pread(self.strtab + sym.n_strx)?;
        Ok((name, sym))
    }
}

impl<'a> Debug for Symbols<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        writeln!(fmt, "Data: {} start: {:#?}, nsyms: {} strtab: {:#x}", self.data.len(), self.start, self.nsyms, self.strtab)?;
        writeln!(fmt, "Symbols: {{")?;
        for (i, res) in self.iter().enumerate() {
            match res {
                Ok((name, nlist)) => {
                    writeln!(fmt, "{: >10x} {} sect: {:#x} type: {:#02x} desc: {:#03x}", nlist.n_value, name, nlist.n_sect, nlist.n_type, nlist.n_desc)?;
                },
                Err(error) => {
                    writeln!(fmt, "  Bad symbol, index: {}, sym: {:?}", i, error)?;
                }
            }
        }
        writeln!(fmt, "}}")
    }
}
