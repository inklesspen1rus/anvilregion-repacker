use std::{
    io::{stdin, stdout, BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, ensure, Context};
use chunk::ChunkData;
use clap::Parser;
use flate2::Compression;
use region::{ChunkInfo, RegionInfo, RegionReader};
use tap::Pipe;
use zerocopy::{
    BigEndian, FromBytes, FromZeros, Immutable, IntoBytes, LittleEndian, TryFromBytes, U32, U64
};

mod chunk;
mod region;

#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
struct BinHeader {
    pub pos: U32<LittleEndian>,
    pub timestamp: U32<BigEndian>,
    pub length: U64<LittleEndian>,
}

#[derive(Debug, Parser)]
struct Cli {
    /// Input file
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Output file. Required for decompacting
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(short)]
    pub compact: bool,

    #[arg(short)]
    pub decompact: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    ensure!(
        args.compact != args.decompact || !args.compact,
        "Must be specified only a single operation!"
    );
    ensure!(
        args.compact != args.decompact || args.compact,
        "Operation must be specified!"
    );

    if args.compact {
        let input = args
            .input
            .context("Input file must be specified when compacting")?;

        compact_file(input, args.output)?;
    } else {
        let output = args
            .output
            .context("Output file must be specified when decompacting")?;

        decompact_file(args.input, output)?;
    }

    Ok(())
}

fn decompact_file(input: Option<impl AsRef<Path>>, output: impl AsRef<Path>) -> anyhow::Result<()> {
    let mut reader: BufReader<Box<dyn Read>> = if let Some(input) = input {
        std::fs::File::open(input)
            .map(Box::new)
            .map(|x| x as Box<dyn Read>)
            .map(|x| std::io::BufReader::with_capacity(4096, x))?
    } else {
        (Box::new(stdin()) as Box<dyn Read>).pipe(|x| BufReader::with_capacity(4096, x))
    };

    let mut writer = std::fs::File::options()
        .write(true)
        .create(true)
        .open(output.as_ref())
        .map(BufWriter::new)?;

    decompact_ws(&mut reader, &mut writer)
        .and_then(|_| writer.flush().context("Unable to flush file"))
        .context("Unable to decompact region")
        .inspect_err(|_| {
            std::fs::remove_file(output)
                .inspect_err(|e| eprintln!("{e}"))
                .ok();
        })?;

    Ok(())
}

fn compact_file(input: impl AsRef<Path>, output: Option<impl AsRef<Path>>) -> anyhow::Result<()> {
    let mut reader = std::fs::File::open(input.as_ref())?.pipe(std::io::BufReader::new);

    let mut writer: BufWriter<Box<dyn Write>> = if let Some(output_file) = output.as_ref() {
        std::fs::File::options()
            .write(true)
            .create(true)
            .open(output_file)?
            .pipe(Box::new)
            .pipe(|x| x as Box<dyn Write>)
            .pipe(std::io::BufWriter::new)
    } else {
        (Box::new(stdout()) as Box<dyn Write>).pipe(BufWriter::new)
    };

    if let Err(e) = compact(&mut reader, &mut writer).context(anyhow!(
        "{:?}",
        output.as_ref().map(|x| x.as_ref().display().to_string())
    )) {
        writer.flush().ok();
        drop(writer);

        if let Some(output) = output {
            let rf = std::fs::remove_file(output);
            if rf.is_err() {
                rf.context(anyhow!(e))?;
            } else {
                bail!(e);
            }
        } else {
            bail!(e);
        }
    } else {
        writer.flush()?;
        drop(writer);
    }

    Ok(())
}

fn compact(reader: impl Read, mut writer: impl Write) -> anyhow::Result<u64> {
    let mut regionreader = RegionReader::from_reader(reader)?;

    // We need aligned reading due to ChunkData layout
    let mut chunkbuf = Vec::<u32>::new();
    let mut databuf = vec![];
    let mut total_written = 0u64;
    loop {
        let Some((info, pos)) = regionreader.next_chunk_info() else {
            break;
        };

        chunkbuf.extend((chunkbuf.len()..info.size().div_ceil(4) as usize).map(|_| 0));
        let Some(_) = regionreader.read_next_chunk(chunkbuf.as_mut_slice().as_mut_bytes())? else {
            break;
        };

        let data =
            ChunkData::try_ref_from_bytes(chunkbuf.as_bytes()).map_err(|x| x.map_src(|_| &()))?;

        data.decompress(&mut databuf)?;

        let header = BinHeader {
            pos: (pos as u32).into(),
            timestamp: info.timestamp,
            length: (databuf.len() as u64).into(),
        };

        writer.write_all(header.as_bytes())?;
        writer.write_all(&databuf)?;
        total_written += header.as_bytes().len() as u64 + databuf.len() as u64;

        databuf.clear();
    }

    Ok(total_written)
}

fn decompact_ws(mut reader: impl Read, mut writer: impl Write + Seek) -> anyhow::Result<u64> {
    let mut chunkinfos = vec![None; 1024];
    let mut header = BinHeader::new_zeroed();
    let mut buffer = vec![];
    let mut buffer2 = vec![];

    writer.seek(std::io::SeekFrom::Start(RegionInfo::SIZE as u64))?;
    let mut location = RegionInfo::SIZE as u64;

    loop {
        let ret = reader.read_exact(header.as_mut_bytes());
        if ret
            .as_ref()
            .is_err_and(|e| e.kind() == std::io::ErrorKind::UnexpectedEof)
        {
            writer.seek(std::io::SeekFrom::Start(0))?;

            chunkinfos
                .iter()
                .map(|x| x.as_ref().map(|x: &ChunkInfo| x.locdata.get()).unwrap_or(FromZeros::new_zeroed()))
                .try_for_each(|x| writer.write_all(x.as_bytes()))?;

            chunkinfos
                .iter()
                .map(|x| x.as_ref().map(|x: &ChunkInfo| x.timestamp).unwrap_or(FromZeros::new_zeroed()))
                .try_for_each(|x| writer.write_all(x.as_bytes()))?;

            return Ok(location);
        }
        ret?;

        let copied = std::io::copy(&mut reader.by_ref().take(header.length.get()), &mut buffer)?;
        ensure!(
            copied == header.length.get(),
            std::io::Error::from(std::io::ErrorKind::UnexpectedEof)
        );

        let mut compreader = flate2::read::ZlibEncoder::new(&buffer[..], Compression::new(3));
        let compressed_size =
            std::io::copy(&mut compreader, &mut buffer2).context("Compression/write failed")?;

        let data_size = compressed_size + 5;

        writer.write_all(U32::<BigEndian>::new((data_size - 4) as u32).as_bytes())?;
        writer.write_all(2u8.as_bytes())?;
        writer.write_all(&buffer2)?;

        const COPIED_MASK: u64 = const { ChunkInfo::SECTOR_SIZE as u64 - 1 };
        let left = (ChunkInfo::SECTOR_SIZE as u64 - (data_size & COPIED_MASK)) & COPIED_MASK;
        writer.seek(std::io::SeekFrom::Current(left as i64))?;

        let chunkinfo = Some(ChunkInfo::new(
            location.try_into().unwrap(),
            (data_size + left).try_into().unwrap(),
            header.timestamp.get(),
        ));
        let old = core::mem::replace(&mut chunkinfos[header.pos.get() as usize], chunkinfo);
        debug_assert!(old.is_none());

        location += data_size + left;

        buffer.clear();
        buffer2.clear();
    }
}
