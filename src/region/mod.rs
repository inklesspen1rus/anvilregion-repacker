#![allow(unused)]

use anyhow::Context;
use core::{
    fmt::Debug,
    num::{NonZeroU32, NonZeroU64},
};
use std::io::{Read, Write};
use zerocopy::{try_transmute, BigEndian, IntoBytes, TryFromBytes, U32};

#[derive(TryFromBytes, Clone, Copy)]
#[repr(C)]
#[non_exhaustive]
pub struct ChunkInfo {
    /// Actually, U32<BigEndian> but NonZeroU32 save Option<ChunkInfo> from bloat
    /// so size_of::<ChunkInfo>() == size_of::<Option<ChunkInfo>>()
    pub locdata: NonZeroU32,
    pub timestamp: U32<BigEndian>,
}

impl Debug for ChunkInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkInfo")
            .field("location", &self.location())
            .field("size", &self.size())
            .field("(locdata: U32BE, timestamp: U32BE)", &(self.locdata, self.timestamp))
            .finish()
    }
}

impl ChunkInfo {
    pub const SECTOR_SIZE: u16 = 4096;

    pub fn new(location: NonZeroU64, size: NonZeroU64, timestamp: u32) -> Self {
        let location = location.get();
        let size = size.get();

        assert!(location % Self::SECTOR_SIZE as u64 == 0, "Location must be mod of {}", Self::SECTOR_SIZE);
        assert!(size % Self::SECTOR_SIZE as u64 == 0, "Size must be mod of {}", Self::SECTOR_SIZE);
        assert!(size / 4096 <= 0xFF, "Size must be less or equal than 1 MiB");

        let mut locdata = U32::<BigEndian>::new(0);
        let locdata_bytes = locdata.as_mut_bytes();

        let location = U32::<BigEndian>::new((location / 4096) as u32);
        let location_bytes = location.as_bytes();
        locdata_bytes[0] = location_bytes[1];
        locdata_bytes[1] = location_bytes[2];
        locdata_bytes[2] = location_bytes[3];
        locdata_bytes[3] = (size / 4096) as u8;

        Self {
            locdata: try_transmute!(locdata).unwrap(),
            timestamp: U32::<BigEndian>::new(timestamp),
        }
    }

    pub fn location(&self) -> u64 {
        let locdata_bytes = self.locdata.as_bytes();
        let location =
            u32::from_be_bytes([0, locdata_bytes[0], locdata_bytes[1], locdata_bytes[2]]);

        location as u64 * Self::SECTOR_SIZE as u64
    }

    pub fn size(&self) -> u64 {
        let locdata_bytes = self.locdata.as_bytes();
        locdata_bytes[3] as u64 * Self::SECTOR_SIZE as u64
    }
}

#[derive(Debug, Clone)]
pub struct RegionInfo(Vec<(ChunkInfo, u16)>);

impl RegionInfo {
    pub const SIZE: u16 = 8192;
    pub const MAX_CHUNK_COUNT: u16 = 1024;

    pub fn read(mut reader: impl Read) -> anyhow::Result<Self> {
        let mut v = vec![0u32; 1024 * 2];
        reader.read_exact(v.as_mut_bytes())?;

        let (locdatas, timestamps) = v.split_at(1024);
        let mut chunks: Vec<(ChunkInfo, u16)> = locdatas
            .into_iter()
            .copied()
            .zip(timestamps.into_iter().copied())
            .zip(0..)
            .filter_map(|((a, b), pos)| try_transmute!([a, b]).ok().map(|x| (x, pos as u16)))
            .collect();

        chunks.sort_by_key(|x| x.0.location());
        Ok(Self(chunks))
    }

    pub fn chunk_infos(&self) -> &[(ChunkInfo, u16)] {
        self.0.as_slice()
    }
}

#[derive(Debug, Clone)]
pub struct RegionReader<R> {
    reader: R,
    info: RegionInfo,
    pos: u64,
    next_chunk: u16,
    tainted: bool,
}

impl<R: Read> RegionReader<R> {
    pub fn from_reader(mut reader: R) -> anyhow::Result<Self> {
        let info = RegionInfo::read(&mut reader)?;

        Ok(Self {
            reader,
            info,
            pos: 8192,
            next_chunk: 0,
            tainted: false,
        })
    }

    pub fn next_chunk_info(&self) -> Option<(ChunkInfo, u16)> {
        self.info
            .chunk_infos()
            .get(self.next_chunk as usize)
            .map(|x| *x)
    }

    /// # Errors
    /// If this method gives error, the reader being tainted and must be dropped. Buffer will contain a trash.
    /// Next call of this method will panic.
    #[track_caller]
    pub fn read_next_chunk(&mut self, mut writer: impl Write) -> anyhow::Result<Option<(ChunkInfo, u64)>> {
        if self.tainted {
            panic!("RegionReader is tainted");
        }

        let Some((nextinfo, _)) = self.next_chunk_info() else {
            return Ok(None);
        };

        let location = nextinfo.location();
        assert!(self.pos <= location);

        if location != self.pos {
            self.reader
                .readskip(location - self.pos)
                .inspect_err(|_| self.tainted = true)?;
            self.pos = location;
        }

        let size = nextinfo.size();
        let copied = std::io::copy(&mut self.reader.by_ref().take(size), &mut writer)
            .inspect_err(|_| self.tainted = true)
            .context("asd")?;

        self.pos += copied;
        self.next_chunk += 1;

        Ok(Some((nextinfo, copied)))
    }
}

trait ReadSkip {
    fn readskip(&mut self, count: u64) -> std::io::Result<()>;
}

impl<R: Read> ReadSkip for R {
    fn readskip(&mut self, count: u64) -> std::io::Result<()> {
        let mut buf = [0u8; 512];

        let rounds = count / 512;
        let remaining = count % 512;

        for _ in 0..rounds {
            self.read_exact(&mut buf)?;
        }

        if remaining != 0 {
            self.read_exact(&mut buf[0..remaining as usize])?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use bytes::{BufMut, BytesMut};
    use zerocopy::IntoBytes;

    use crate::region::ChunkInfo;

    use super::RegionReader;

    #[test]
    fn chunk_info_new() {
        let locdata = [[0u8, 0, 16, 2]; 1024];
        let timestamp = [[0u8, 0, 1, 0]; 1024];

        let mut buf = BytesMut::new();
        (&mut buf).writer().write_all(locdata.as_bytes()).unwrap();
        (&mut buf).writer().write_all(timestamp.as_bytes()).unwrap();

        let region_reader =  RegionReader::from_reader(&buf[..]).unwrap();

        let info = region_reader.next_chunk_info().unwrap();

        assert_eq!(info.0.location(), 16 * 4096);
        assert_eq!(info.0.size(), 2 * 4096);
        assert_eq!(info.0.timestamp.get(), 256);

        assert_eq!(info.1, 0);

        let info = ChunkInfo::new(info.0.location().try_into().unwrap(), info.0.size().try_into().unwrap(), info.0.timestamp.get());

        assert_eq!(info.location(), 16 * 4096);
        assert_eq!(info.size(), 2 * 4096);
        assert_eq!(info.timestamp.get(), 256);
    }
}
