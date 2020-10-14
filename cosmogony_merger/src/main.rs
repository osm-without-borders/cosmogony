use cosmogony::{file_format::OutputFormat, read_zones_from_file, Zone, ZoneIndex};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Args {
    /// Cosmogony files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
    /// output file name
    #[structopt(
        short = "o",
        long = "output",
        default_value = "cosmogony.json",
        help = r#"Output file name. Format will be deduced from the file extension.
    Accepted extensions are '.jsonl', '.jsonl.gz'
    'jsonl' is json stream, each line is a zone as json
    "#
    )]
    output: String,
}

#[derive(Default)]
struct CosmogonyMerger {
    id_offset: usize,
}

fn to_json_stream(
    mut writer: impl std::io::Write,
    zones: impl std::iter::Iterator<Item = Zone>,
) -> Result<(), failure::Error> {
    for z in zones {
        serde_json::to_writer(&mut writer, &z)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

impl CosmogonyMerger {
    fn merge_cosmogony(
        &mut self,
        files: &[PathBuf],
        mut writer: impl std::io::Write,
    ) -> Result<(), failure::Error> {
        for f in files {
            self.read_cosmogony(&f, &mut writer)?;
        }
        Ok(())
    }

    fn read_cosmogony(
        &mut self,
        file: &PathBuf,
        writer: impl std::io::Write,
    ) -> Result<(), failure::Error> {
        let mut max_id = 0;
        let zones = read_zones_from_file(file)?
            .into_iter()
            .filter_map(|z| z.ok())
            .map(|mut z| {
                z.id = self.get_updated_id(z.id);
                max_id = std::cmp::max(max_id, z.id.index);
                z.parent = z.parent.map(|p| self.get_updated_id(p));
                z
            });
        to_json_stream(writer, zones)?;
        // we update the id_offset, for the next file
        self.id_offset = max_id + 1;
        Ok(())
    }

    fn get_updated_id(&self, idx: ZoneIndex) -> ZoneIndex {
        ZoneIndex {
            index: idx.index + self.id_offset,
        }
    }
}

fn merge_cosmogony(args: Args) -> Result<(), failure::Error> {
    let mut merger = CosmogonyMerger::default();

    let format = OutputFormat::from_filename(&args.output)?;
    let file = std::fs::File::create(args.output)?;
    let mut stream = std::io::BufWriter::new(file);
    match format {
        OutputFormat::JsonGz | OutputFormat::Json => panic!(
            "cannot read real cosmogonies, only jsonl/jsonl.gz to be able to stream the files"
        ),
        OutputFormat::JsonStream => {
            merger.merge_cosmogony(&args.files, &mut stream)?;
        }
        OutputFormat::JsonStreamGz => {
            let mut e = GzEncoder::new(stream, Compression::default());
            merger.merge_cosmogony(&args.files, &mut e)?;
        }
    };
    Ok(())
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
    let args = Args::from_args();
    if let Err(e) = merge_cosmogony(args) {
        log::error!("impossible to merge cosmogony: {:?}", e);
        e.iter_chain().for_each(|c| {
            log::error!("{}", c);
            if let Some(b) = c.backtrace() {
                log::error!("  - {}", b);
            }
        });
        std::process::exit(1);
    }
}
