use anyhow::bail;
use bytes::Buf;
use core::fmt::Debug;
use std::io::Write;
use zerocopy::{BigEndian, FromBytes, FromZeros, Immutable, KnownLayout, U32};

#[derive(FromBytes, KnownLayout, Immutable)]
#[repr(C, align(4))]
pub struct ChunkData {
    length: U32<BigEndian>,
    pub compression_type: u8,
    pub data: [u8],
}

impl ChunkData {
    pub fn length(&self) -> usize {
        (self.length.get() - 1) as usize
    }

    pub fn decompress(&self, mut writer: impl Write) -> anyhow::Result<usize> {
        let (data, _) = self
            .data
            .split_at_checked(self.length())
            .ok_or(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;

        if self.compression_type == CompressionType::Zlib as u8 {
            let mut decompressor = flate2::read::ZlibDecoder::new(data);
            let copied = std::io::copy(&mut decompressor, &mut writer)?;
            Ok(copied as usize)
        } else if self.compression_type == CompressionType::GZip as u8 {
            let mut decompressor = flate2::read::GzDecoder::new(data);
            let copied = std::io::copy(&mut decompressor, &mut writer)?;
            Ok(copied as usize)
        } else if self.compression_type == CompressionType::Uncompressed as u8 {
            let copied = std::io::copy(&mut &data[..], &mut writer)?;
            Ok(copied as usize)
        } else {
            bail!("Unknown compression type {}", self.compression_type)
        }
    }
}

impl Debug for ChunkData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkData")
            .field("length()", &self.length())
            .field("compression_type", &self.compression_type)
            .field("raw_length", &self.length)
            .field("data.len()", &self.data.len())
            .finish()
    }
}

#[derive(Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum CompressionType {
    GZip = 1,
    Zlib = 2,
    Uncompressed = 3,
    LZ4 = 4,
    Custom = 127,
}
