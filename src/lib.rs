//! fit2gpx
//!
//! a simple fit to gpx converter,
//! with a feature for adding elevation from `srtm` data
//!
//!
//!
//!
//!
//!
//!
//!
//!
// TODO: proper docs

pub use fit::Fit;

/// universal Result, but not sendable
pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg(feature = "elevation")]
pub mod elevation;
pub mod fit;
mod utils;
