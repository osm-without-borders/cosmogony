extern crate cosmogony;
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

#[derive(StructOpt, Debug)]
struct Args {
    /// OSM PBF file.
    #[structopt(short = "i", long = "input")]
    input: String,
    /// output file name
    #[structopt(short = "o", long = "output", default_value = "cosmogony.json")]
    output: String,
}

fn serialize_to_json(cosmogony: &Cosmogony, output_file: String) {
    let json = serde_json::to_string(cosmogony).unwrap();

    let mut file = File::create(output_file).unwrap();
    file.write_all(json.as_bytes()).unwrap();
}

fn main() {
    mimir::logger_init();
    let args = Args::from_args();
    let cosmogony = build_cosmogony(args.input);

    serialize_to_json(&cosmogony, args.output);
}
