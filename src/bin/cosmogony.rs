extern crate cosmogony;
extern crate failure;
#[macro_use]
extern crate log;
extern crate mimir;
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
}

fn serialize_to_json(cosmogony: &Cosmogony, output_file: String) -> Result<(), Error> {
    let json = serde_json::to_string(cosmogony)?;

    let mut file = File::create(output_file)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

fn cosmogony(args: Args) -> Result<(), Error> {
    let cosmogony = build_cosmogony(args.input)?;

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
    mimir::logger_init();
    let args = Args::from_args();
    match cosmogony(args) {
        Err(e) => {
            error!("error in cosmogony: {:?}", e);
            std::process::exit(1);
        }
        _ => (),
    }
}
