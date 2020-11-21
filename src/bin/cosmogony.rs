use cosmogony::{file_format::OutputFormat, Cosmogony};
use cosmogony_builder::{build_cosmogony, merger};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use structopt::StructOpt;

/// Cosmogony arguments
///
/// You can:
///
/// * generate a cosmogony file from an osm file (generate)
///
/// * merge several cosmogonies into one (merge)
///
/// Note: for retrocompatibility, if no subcommand is provided, the default one is `generate`
///
/// So `cosmogony -i <osm-file> -o output file` if the same as
/// `cosmogony generate -i <osm-file> -o output file`
#[derive(StructOpt, Debug)]
enum Args {
    /// Generate cosmogony subcommand
    ///
    /// Note: for retrocompatibility this is also the default subcommand if none is provided
    #[structopt(name = "generate")]
    Generate(GenerateArgs),
    /// Merge cosmogony subcommand
    ///
    /// Use it to merge several streamed cosmogony files into one.
    /// Can be useful to split the processing of a large osm file (like the planet)
    /// into several non overlapping small ones
    #[structopt(name = "merge")]
    Merge(MergeArgs),
}

#[derive(StructOpt, Debug)]
struct GenerateArgs {
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
    output: String,
    #[structopt(help = "Do not display the stats", long = "no-stats")]
    no_stats: bool,
    #[structopt(
        help = "country code if the pbf file does not contains any country",
        long = "country-code"
    )]
    country_code: Option<String>,
    #[structopt(
        help = "Prevent voronoi geometries computation and generation",
        long = "disable-voronoi"
    )]
    disable_voronoi: bool,
    #[structopt(help = "Only generates labels for given langs", long = "filter-langs")]
    filter_langs: Vec<String>,
}

#[derive(StructOpt, Debug)]
struct MergeArgs {
    /// Cosmogony files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
    /// output file name
    #[structopt(
        short = "o",
        long = "output",
        default_value = "cosmogony.jsonl",
        help = r#"Output file name. Format will be deduced from the file extension.
    Accepted extensions are '.jsonl', '.jsonl.gz' (no json or json.gz)
    'jsonl' is json stream, each line is a zone as json
    "#
    )]
    output: PathBuf,
}

fn to_json_stream(
    mut writer: impl std::io::Write,
    cosmogony: &Cosmogony,
) -> Result<(), failure::Error> {
    for z in &cosmogony.zones {
        serde_json::to_writer(&mut writer, z)?;
        writer.write_all(b"\n")?;
    }

    // since we don't dump the metadata in json stream for the moment, we log them
    log::info!("metadata: {:?}", &cosmogony.meta);
    Ok(())
}

fn serialize_cosmogony(
    cosmogony: &Cosmogony,
    output_file: String,
    format: OutputFormat,
) -> Result<(), failure::Error> {
    log::info!("writing the output file {}", output_file);
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

fn cosmogony(args: GenerateArgs) -> Result<(), failure::Error> {
    let format = OutputFormat::from_filename(&args.output)?;

    let cosmogony = build_cosmogony(
        args.input,
        args.country_code,
        args.disable_voronoi,
        &args.filter_langs,
    )?;

    serialize_cosmogony(&cosmogony, args.output, format)?;

    if !args.no_stats {
        log::info!(
            "Statistics for {}:\n{}",
            cosmogony.meta.osm_filename,
            cosmogony.meta.stats
        );
    }
    Ok(())
}

fn run(args: Args) -> Result<(), failure::Error> {
    match args {
        Args::Merge(merge_args) => merger::merge_cosmogony(&merge_args.files, &merge_args.output),
        Args::Generate(gen_args) => cosmogony(gen_args),
    }
}

fn init_logger() {
    let mut builder = env_logger::Builder::new();
    builder.filter(None, log::LevelFilter::Info);
    if let Ok(s) = std::env::var("RUST_LOG") {
        builder.parse_filters(&s);
    }
    builder.init();
}

fn main() {
    init_logger();
    // Note: for retrocompatibility, we also try to read the args without subcommand
    // to generate a cosmogony
    let args = GenerateArgs::from_args_safe()
        .map(|a| Args::Generate(a))
        .unwrap_or_else(|_| Args::from_args());
    if let Err(e) = run(args) {
        log::error!("cosmogony in error! {:?}", e);
        e.iter_chain().for_each(|c| {
            log::error!("{}", c);
            if let Some(b) = c.backtrace() {
                log::error!("  - {}", b);
            }
        });
        std::process::exit(1);
    }
}
