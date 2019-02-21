#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt_derive;
use cosmogony::cosmogony::Cosmogony;
use cosmogony::{build_cosmogony, file_format::OutputFormat};
use failure::Error;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::BufWriter;
use structopt::StructOpt;

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
        help = r#"Output file name. Format will be deduced from the file extension. 
Accepted extensions are '.json', '.json.gz', '.jsonl', '.jsonl.gz'
'jsonl' is json stream, each line is a zone as json
"#
    )]
    output: Option<String>,
    #[structopt(help = "Do not display the stats", long = "no-stats")]
    no_stats: bool,
    #[structopt(
        help = "Do not read the geometry of the boundaries",
        long = "disable-geom"
    )]
    disable_geom: bool,
    #[structopt(
        help = "country code if the pbf file does not contains any country",
        long = "country-code"
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

fn to_json_stream(mut writer: impl std::io::Write, cosmogony: &Cosmogony) -> Result<(), Error> {
    for z in &cosmogony.zones {
        serde_json::to_writer(&mut writer, z)?;
        writer.write(b"\n")?;
    }

    // since we don't dump the metadata in json stream for the moment, we log them
    info!("metadata: {:?}", &cosmogony.meta);
    Ok(())
}

fn serialize_cosmogony(
    cosmogony: &Cosmogony,
    output_file: String,
    format: OutputFormat,
) -> Result<(), Error> {
    info!("writing the output file {}", output_file);
    let file = File::create(output_file)?;
    let stream = BufWriter::new(file);
    match format {
        OutputFormat::JsonGz => {
            let e = GzEncoder::new(stream, Compression::default());
            serde_json::to_writer(e, cosmogony)?;
        }
        OutputFormat::Json => {
            serde_json::to_writer(stream, cosmogony)?;
        }
        OutputFormat::JsonStream => {
            to_json_stream(stream, cosmogony)?;
        }
        OutputFormat::JsonStreamGz => {
            let e = GzEncoder::new(stream, Compression::default());
            to_json_stream(e, cosmogony)?;
        }
    };
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
            e.iter_chain().for_each(|c| {
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
