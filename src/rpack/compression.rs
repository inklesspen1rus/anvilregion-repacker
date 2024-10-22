use clap::ValueEnum;
use std::io::{Read, Write};
use tap::Pipe;
use zerocopy::{FromZeros, Immutable, IntoBytes, KnownLayout, TryFromBytes};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ValueEnum,
    FromZeros,
    IntoBytes,
    Immutable,
    KnownLayout,
)]
#[repr(u8)]
#[non_exhaustive]
pub enum CompactCompression {
    Zstd = 1,
    LZ4 = 2,
    None = 0,
}

impl CompactCompression {
    pub fn compress(&self, mut reader: impl Read, mut writer: impl Write) -> anyhow::Result<u64> {
        match *self {
            CompactCompression::None => std::io::copy(&mut reader, &mut writer)?,
            CompactCompression::LZ4 => {
                let mut writer =
                    lz4_flex::frame::FrameEncoder::new(count_write::CountWrite::from(&mut writer));
                std::io::copy(&mut reader, &mut writer)?;
                let mut writer = writer.finish()?;
                writer.flush()?;
                writer.count()
            }
            CompactCompression::Zstd => {
                let mut reader = zstd::stream::read::Encoder::new(&mut reader, 3)?;
                std::io::copy(&mut reader, &mut writer)?
            }
        }
        .pipe(Ok)
    }

    pub fn decompress(&self, mut reader: impl Read, mut writer: impl Write) -> anyhow::Result<u64> {
        match *self {
            CompactCompression::None => std::io::copy(&mut reader, &mut writer)?,
            CompactCompression::LZ4 => {
                let mut reader = lz4_flex::frame::FrameDecoder::new(&mut reader);
                std::io::copy(&mut reader, &mut writer)?
            }
            CompactCompression::Zstd => {
                let mut reader = zstd::stream::read::Decoder::new(&mut reader)?;
                std::io::copy(&mut reader, &mut writer)?
            }
        }
        .pipe(Ok)
    }

    pub fn decoder<'a>(&self, reader: impl Read + 'a) -> anyhow::Result<Box<dyn Read + 'a>> {
        match *self {
            CompactCompression::None => Box::new(reader) as Box<dyn Read>,
            CompactCompression::LZ4 => Box::new(lz4_flex::frame::FrameDecoder::new(reader)),
            CompactCompression::Zstd => Box::new(zstd::Decoder::new(reader)?),
        }.pipe(Ok)
    }

    pub fn encoder<'a>(&self, writer: impl Write + 'a) -> anyhow::Result<Box<dyn Write + 'a>> {
        match *self {
            CompactCompression::None => Box::new(writer) as Box<dyn Write>,
            CompactCompression::LZ4 => Box::new(lz4_flex::frame::FrameEncoder::new(writer)),
            CompactCompression::Zstd => Box::new(zstd::Encoder::new(writer, 3)?),
        }.pipe(Ok)
    }
}
