extern crate cosmogony;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate log;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use std::fs::File;
use std::io::prelude::*;
use cosmogony::build_cosmogony;
use cosmogony::cosmogony::Cosmogony;
use structopt::StructOpt;

use failure::Error;

#[derive(StructOpt, Debug)]
struct Args {
    /// OSM PBF file.
    #[structopt(short = "i", long = "input")]
    input: String,
    /// output file name
    #[structopt(short = "o", long = "output")]
    output: Option<String>,
    #[structopt(long = "print-stats", default_value = "true")]
    print_stats: bool,
    #[structopt(help = "Do not read the geometry of the boundaries", long = "disable-geom")]
    disable_geom: bool,
    #[structopt(help = "country code if the pbf file does not contains any country",
                long = "country-code")]
    country_code: Option<String>,
    #[structopt(help = "libpostal path", long = "libpostal")]
    libpostal_path: String,
}

fn serialize_to_json(cosmogony: &Cosmogony, output_file: String) -> Result<(), Error> {
    let json = serde_json::to_string(cosmogony)?;

    let mut file = File::create(output_file)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

fn cosmogony(args: Args) -> Result<(), Error> {
    let cosmogony = build_cosmogony(
        args.input,
        !args.disable_geom,
        args.libpostal_path.into(),
        args.country_code,
    )?;

    if let Some(output) = args.output {
        serialize_to_json(&cosmogony, output)?;
    }

    if args.print_stats {
        println!(
            "Statistics for {}:\n{}",
            cosmogony.meta.osm_filename, cosmogony.meta.stats
        );
    }

    Ok(())
}

fn main() {
    env_logger::init();
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
