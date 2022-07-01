use anyhow::{anyhow, Result};
use clap::ErrorKind;
use clap::Parser;
use cosmogony::{file_format::OutputFormat, Cosmogony};
use cosmogony_builder::{build_cosmogony, merger};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

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
#[derive(Debug, clap::Parser)]
#[clap(version)]
enum Args {
    /// Generate cosmogony subcommand
    ///
    /// Note: for retrocompatibility this is also the default subcommand if none is provided
    #[clap(name = "generate")]
    Generate(GenerateArgs),
    /// Merge cosmogony subcommand
    ///
    /// Use it to merge several streamed cosmogony files into one.
    /// Can be useful to split the processing of a large osm file (like the planet)
    /// into several non overlapping small ones
    #[clap(name = "merge")]
    Merge(MergeArgs),
}

#[derive(Debug, clap::Parser)]
struct GenerateArgs {
    /// OSM PBF file.
    #[clap(short, long)]
    input: String,
    /// output file name
    #[clap(
        short,
        long,
        default_value = "cosmogony.json",
        help = concat!(
            "Output file name. Format will be deduced from the file extension. ",
            "Accepted extensions are '.json', '.json.gz', '.jsonl', '.jsonl.gz'. ",
            "'jsonl' is json stream where each line is a zone as json.",
        )
    )]
    output: String,
    #[clap(help = "Do not display the stats", long)]
    no_stats: bool,
    #[clap(
        help = "Country code if the pbf file does not contains any country",
        long
    )]
    country_code: Option<String>,
    #[clap(
        help = "Prevent voronoi geometries computation and generation",
        long = "disable-voronoi"
    )]
    disable_voronoi: bool,
    #[clap(
        help = concat!(
            "Only generates labels for given langs. ",
            "Either repeat parameter or use comma-separated value.",
        ),
        long = "filter-langs"
    )]
    filter_langs_raw: Vec<String>,
    #[clap(
        help = concat!(
            "Configure the max number of threads using during computations. ",
            "This won't affect the number of threads used to parse the OSM file.",
        ),
        long
    )]
    num_threads: Option<usize>,
}

impl GenerateArgs {
    fn filter_langs(&self) -> Vec<String> {
        self.filter_langs_raw
            .iter()
            .flat_map(|val| val.split(',').map(String::from))
            .collect()
    }
}

#[derive(Debug, clap::Parser)]
struct MergeArgs {
    /// Cosmogony files to process
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
    /// output file name
    #[clap(
        short = 'o',
        long = "output",
        default_value = "cosmogony.jsonl",
        help = r#"Output file name. Format will be deduced from the file extension.
    Accepted extensions are '.jsonl', '.jsonl.gz' (no json or json.gz)
    'jsonl' is json stream, each line is a zone as json
    "#
    )]
    output: PathBuf,
}

fn to_json_stream(mut writer: impl std::io::Write, cosmogony: &Cosmogony) -> Result<()> {
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
) -> Result<()> {
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

fn cosmogony(args: GenerateArgs) -> Result<()> {
    let format = OutputFormat::from_filename(&args.output)?;
    let filter_langs = args.filter_langs();
    println!("{:?}", filter_langs);

    if let Some(num_threads) = args.num_threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
            .map_err(|err| anyhow!("could not init rayon's global thread pool: {err}"))?;
    }

    let cosmogony = build_cosmogony(
        args.input,
        args.country_code,
        args.disable_voronoi,
        &filter_langs,
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

fn run(args: Args) -> Result<()> {
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
    let args = GenerateArgs::try_parse()
        .map(Args::Generate)
        .unwrap_or_else(|err| {
            if let ErrorKind::DisplayVersion = err.kind() {
                // The version number has been displayed.
                // Args should not be parsed a second time.
                println!();
                std::process::exit(0)
            }
            Args::parse()
        });

    println!("{:?}", args);
    if let Err(e) = run(args) {
        log::error!("cosmogony in error! {:?}", e);
        e.chain().for_each(|c| {
            log::error!("{}", c);
        });
        std::process::exit(1);
    }
}
