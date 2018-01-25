extern crate cosmogony;
extern crate mimir;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use cosmogony::build_cosmogony;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Args {
    /// OSM PBF file.
    #[structopt(short = "i", long = "input")]
    input: String,
}

fn main() {
    mimir::logger_init();
    let args = Args::from_args();
    build_cosmogony(args.input);
}
