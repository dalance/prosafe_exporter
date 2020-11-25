mod exporter;
mod prosafe_switch;

use crate::exporter::Exporter;
use failure::Error;
use structopt::{clap, StructOpt};

// -------------------------------------------------------------------------------------------------
// Opt
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "prosafe_exporter")]
#[structopt(long_version = option_env!("LONG_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))]
#[structopt(setting = clap::AppSettings::ColoredHelp)]
#[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
pub struct Opt {
    /// Address on which to expose metrics and web interface.
    #[structopt(long = "web.listen-address", default_value = ":9493")]
    pub listen_address: String,

    /// Static target probed through /metrics.
    #[structopt(long = "target")]
    pub target: Option<String>,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

#[cfg_attr(tarpaulin, skip)]
fn run() -> Result<(), Error> {
    let opt = Opt::from_args();
    let _ = Exporter::start(&opt.listen_address, opt.target, opt.verbose);
    Ok(())
}

#[cfg_attr(tarpaulin, skip)]
fn main() {
    if let Err(x) = run() {
        println!("{}", x);
    }
}
