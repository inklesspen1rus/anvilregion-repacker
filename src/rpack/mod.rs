//! Layout:
//! \[u8; 32\] of header
//! Then chunks, written continuously, each with the header [RpackChunkHeader] then payload [RpackChunkHeader::length] bytes

use zerocopy::{BigEndian, FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout, LittleEndian, TryFromBytes, Unaligned, U32, U64};

mod compression;
mod rpack_reader;

pub use compression::CompactCompression;

#[derive(Debug, TryFromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct RpackHeader {
    pub compression_type: compression::CompactCompression,
}

impl RpackHeader {
    const SIZE: usize = 32;
}

#[derive(Debug, FromZeros)]
pub struct RpackChunkHeader {
    pub pos: U32<LittleEndian>,
    pub timestamp: U32<BigEndian>,
    pub length: U64<LittleEndian>,
}

