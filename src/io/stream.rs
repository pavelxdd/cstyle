use super::encoding::DecodedSource;
use crate::config::FormatOptions;
use std::io::{self, Read, Write};

pub(crate) fn format_reader_to_writer<R, W>(
    mut reader: R,
    mut writer: W,
    options: &FormatOptions,
) -> io::Result<()>
where
    R: Read,
    W: Write,
{
    let mut input = Vec::new();
    reader.read_to_end(&mut input)?;
    let input = DecodedSource::from_bytes(&input)?;
    writer.write_all(&input.format(options))
}
