extern crate bincode;
extern crate combine;
#[macro_use]
extern crate failure;
extern crate hyper;
extern crate interfaces2 as interfaces;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;
extern crate rand;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;
extern crate toml;

mod exporter;
mod prosafe_switch;

use exporter::{Config, Exporter};
use failure::{Error, ResultExt};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use structopt::{clap, StructOpt};

// -------------------------------------------------------------------------------------------------
// Opt
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "prosafe_exporter")]
#[structopt(
    raw(long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))")
)]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
#[structopt(raw(setting = "clap::AppSettings::DeriveDisplayOrder"))]
pub struct Opt {
    /// Config file
    #[structopt(long = "path.config", parse(from_os_str))]
    pub config: PathBuf,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn run() -> Result<(), Error> {
    let opt = Opt::from_args();

    let mut f =
        File::open(&opt.config).context(format!("failed to open file: {:?}", opt.config))?;
    let mut s = String::new();
    let _ = f.read_to_string(&mut s);
    let config: Config = toml::from_str(&s)?;

    let _ = Exporter::start(config, opt.verbose);

    Ok(())
}

fn main() {
    match run() {
        Err(x) => {
            println!("{}", x);
        }
        _ => (),
    }
}
