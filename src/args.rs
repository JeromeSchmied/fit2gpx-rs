use clap::Parser;
use std::path::PathBuf;

/// A converter tool for activity records: from .fit (garmin) to .gpx (xml)
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
#[command(version, about, long_about)]
pub struct Cli {
    /// Path to the Fit files, that shall be converted into their Gpx counterparts
    pub files: Vec<PathBuf>,
    /// Path to the Directory that contains the needed DEM data: .hgt files
    #[cfg(feature = "elevation")]
    #[arg(short = 'd', long, env, default_value = "/tmp")]
    pub elev_data_dir: PathBuf,
    /// Whether elevation data shall be added from .hgt files to each trackpoint
    #[cfg(feature = "elevation")]
    #[arg(short, long, default_value_t = false, requires = "elev_data_dir")]
    pub add_elevation: bool,
    /// Whether already converted Gpx files shall be overwritten
    #[arg(short, long, default_value_t = false)]
    pub overwrite: bool,
}
