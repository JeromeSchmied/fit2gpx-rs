use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone, Debug, PartialEq, Eq)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    pub files: Vec<PathBuf>,
    #[cfg(feature = "elevation")]
    #[arg(short = 'd', long, env, default_value = "/tmp")]
    pub elev_data_dir: PathBuf,
    #[cfg(feature = "elevation")]
    #[arg(short, long, default_value_t = false, requires = "elev_data_dir")]
    pub add_elevation: bool,
    #[arg(short, long, default_value_t = false)]
    pub overwrite: bool,
}
