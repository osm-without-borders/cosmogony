extern crate cosmogony;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate log;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate flate2;

use cosmogony::build_cosmogony;
use cosmogony::cosmogony::Cosmogony;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::prelude::*;
use structopt::StructOpt;

use failure::Error;

#[derive(StructOpt, Debug)]
struct Args {
    /// OSM PBF file.
    #[structopt(short = "i", long = "input")]
    input: String,
    /// output file name
    #[structopt(
        short = "o",
        long = "output",
        default_value = "cosmogony.json",
        help = "Output file name. Format will be deduced from the file extension. Accepted extensions are '.json' and '.json.gz'"
    )]
    output: Option<String>,
    #[structopt(help = "Do not display the stats", long = "no-stats")]
    no_stats: bool,
    #[structopt(help = "Do not read the geometry of the boundaries", long = "disable-geom")]
    disable_geom: bool,
    #[structopt(
        help = "country code if the pbf file does not contains any country", long = "country-code"
    )]
    country_code: Option<String>,
    #[structopt(
        help = "libpostal path",
        long = "libpostal",
        short = "l",
        default_value = "./libpostal/resources/boundaries/osm/"
    )]
    libpostal_path: String,
}

#[derive(PartialEq, Clone)]
enum OutputFormat {
    Json,
    JsonGz,
}

impl OutputFormat {
    fn all_extensions() -> Vec<(String, OutputFormat)> {
        vec![
            (".json".into(), OutputFormat::Json),
            (".json.gz".into(), OutputFormat::JsonGz),
        ]
    }

    fn from_filename(filename: &str) -> Result<OutputFormat, Error> {
        let extensions = OutputFormat::all_extensions();
        extensions
            .iter()
            .find(|&&(ref e, _)| filename.ends_with(e))
            .map(|&(_, ref f)| f.clone())
            .ok_or_else(|| {
                let extensions_str = extensions
                    .into_iter()
                    .map(|(e, _)| e)
                    .collect::<Vec<String>>()
                    .join(", ");
                failure::err_msg(format!(
                    "Unable to detect the file format from filename '{}'. \
                     Accepted extensions are: {}",
                    filename, extensions_str
                ))
            })
    }
}

fn serialize_cosmogony(
    cosmogony: &Cosmogony,
    output_file: String,
    format: OutputFormat,
) -> Result<(), Error> {
    info!("serializing the cosmogony");
    let json = serde_json::to_string(cosmogony)?;
    let output_bytes = match format {
        OutputFormat::JsonGz => {
            let mut e = GzEncoder::new(vec![], Compression::default());
            e.write_all(json.as_bytes())?;
            e.finish()?
        }
        OutputFormat::Json => json.into_bytes(),
    };
    info!("writing the output file {}", output_file);
    let mut file = File::create(output_file)?;
    file.write_all(&output_bytes)?;
    Ok(())
}

fn cosmogony(args: Args) -> Result<(), Error> {
    let format = if let Some(ref output_filename) = args.output {
        OutputFormat::from_filename(&output_filename)?
    } else {
        OutputFormat::Json
    };

    let cosmogony = build_cosmogony(
        args.input,
        !args.disable_geom,
        args.libpostal_path.into(),
        args.country_code,
    )?;

    if let Some(output) = args.output {
        serialize_cosmogony(&cosmogony, output, format)?;
    }

    if !args.no_stats {
        info!(
            "Statistics for {}:\n{}",
            cosmogony.meta.osm_filename, cosmogony.meta.stats
        );
    }
    Ok(())
}

fn init_logger() {
    let mut builder = env_logger::Builder::new();
    builder.filter(None, log::LevelFilter::Info);
    if let Ok(s) = std::env::var("RUST_LOG") {
        builder.parse(&s);
    }
    builder.init();
}

fn main() {
    init_logger();
    let args = Args::from_args();
    match cosmogony(args) {
        Err(e) => {
            error!("cosmogony in error! {:?}", e);
            e.causes().for_each(|c| {
                error!("{}", c);
                if let Some(b) = c.backtrace() {
                    error!("  - {}", b);
                }
            });

            std::process::exit(1);
        }
        _ => (),
    }
}
