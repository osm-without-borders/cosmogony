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
    #[structopt(short = "o", long = "output", default_value = "cosmogony.json")]
    output: String,
}

fn serialize_to_json(cosmogony: &Cosmogony, output_file: String) -> Result<(), Error> {
    let json = serde_json::to_string(cosmogony)?;

    let mut file = File::create(output_file)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

fn cosmogny(args: Args) -> Result<(), Error> {
    let cosmogony = build_cosmogony(args.input)?;

    serialize_to_json(&cosmogony, args.output)?;
    Ok(())
}

fn main() {
    mimir::logger_init();
    let args = Args::from_args();
    match cosmogny(args) {
        Err(e) => {
            error!("error in cosmogony: {:?}", e);
            std::process::exit(1);
        }
        _ => (),
    }
}
