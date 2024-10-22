use core::{marker::{PhantomData, PhantomPinned}, num::NonZeroU64};
use std::{fs::File, io::{self, BufReader, BufWriter, Read, Write}};

use anyhow::Context;
use zerocopy::{FromZeros, IntoBytes, TryFromBytes};

use super::{CompactCompression, RpackChunkHeader, RpackHeader};

pub struct RpackReader<'a, R> {
    header: RpackHeader,
    reader: Box<dyn Read + 'a>,
    _data: PhantomData<&'a R>
}

impl<'a, R: Read> RpackReader<'a, R> {
    /// Recommended to use BufReader
    pub fn from_reader(mut reader: R) -> anyhow::Result<Self>
    {
        let mut header = [0u8; RpackHeader::SIZE];
        reader.read_exact(header.as_mut_bytes())?;

        let (header, _) = RpackHeader::try_read_from_prefix(header.as_slice())
            .map_err(|e| e.map_src(|_| &()))
            .context("Reading Rpack header")?;

        let reader = header.compression_type.decoder(reader)?;

        Ok(Self { header, reader, _data: PhantomData })
    }
    
    /// [None] means there are no chunks more
    pub fn read_chunk(&mut self, mut writer: impl Write) -> anyhow::Result<Option<NonZeroU64>> {
        
        todo!()
    }
}

fn a() -> RpackReader<'static, BufReader<File>> {
    let a = File::open("/dev/zero").unwrap();

    RpackReader::from_reader(BufReader::new(File::open("/dev/zero").unwrap())).unwrap()
}
