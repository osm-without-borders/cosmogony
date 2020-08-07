use crate::file_format::OutputFormat;
use crate::{Cosmogony, Zone};
use failure::Error;
use std::path::Path;

// Stream Cosmogony's Zone from a Reader
fn read_zones(
    reader: impl std::io::BufRead,
) -> impl std::iter::Iterator<Item = Result<Zone, Error>> {
    reader
        .lines()
        .map(|l| l.map_err(|e| failure::err_msg(e.to_string())))
        .map(|l| {
            l.and_then(|l| serde_json::from_str(&l).map_err(|e| failure::err_msg(e.to_string())))
        })
}

fn from_json_stream(reader: impl std::io::BufRead) -> Result<Cosmogony, Error> {
    let zones = read_zones(reader).collect::<Result<_, _>>()?;

    Ok(Cosmogony {
        zones,
        ..Default::default()
    })
}

/// Load a cosmogony from a file
pub fn load_cosmogony_from_file(input: impl AsRef<Path>) -> Result<Cosmogony, Error> {
    let format = OutputFormat::from_filename(input.as_ref())?;
    let f = std::fs::File::open(&input)?;
    let f = std::io::BufReader::new(f);
    load_cosmogony(f, format)
}

/// Return an iterator on the zones
/// if the input file is a jsonstream, the zones are streamed
/// if the input file is a json, the whole cosmogony is loaded
pub fn read_zones_from_file(
    input: impl AsRef<Path>,
) -> Result<Box<dyn std::iter::Iterator<Item = Result<Zone, Error>>>, Error> {
    let format = OutputFormat::from_filename(input.as_ref())?;
    let f = std::fs::File::open(input.as_ref())?;
    let f = std::io::BufReader::new(f);
    match format {
        OutputFormat::JsonGz | OutputFormat::Json => {
            let cosmo = load_cosmogony(f, format)?;
            Ok(Box::new(cosmo.zones.into_iter().map(Ok)))
        }
        OutputFormat::JsonStream => Ok(Box::new(read_zones(f))),
        OutputFormat::JsonStreamGz => {
            let r = flate2::bufread::GzDecoder::new(f);
            let r = std::io::BufReader::new(r);
            Ok(Box::new(read_zones(r)))
        }
    }
}

// Load a cosmogony from a reader and a file_format
fn load_cosmogony(reader: impl std::io::BufRead, format: OutputFormat) -> Result<Cosmogony, Error> {
    match format {
        OutputFormat::JsonGz => {
            let r = flate2::read::GzDecoder::new(reader);
            serde_json::from_reader(r).map_err(|e| failure::err_msg(e.to_string()))
        }
        OutputFormat::Json => {
            serde_json::from_reader(reader).map_err(|e| failure::err_msg(e.to_string()))
        }
        OutputFormat::JsonStream => from_json_stream(reader),
        OutputFormat::JsonStreamGz => {
            let r = flate2::bufread::GzDecoder::new(reader);
            let r = std::io::BufReader::new(r);
            from_json_stream(r)
        }
    }
}
