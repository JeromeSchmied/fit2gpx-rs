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

use crate::utils::*;
use fit_file::{fit_file, FitFieldValue, FitRecordMsg, FitSessionMsg};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::{fs::File, io::BufWriter, path::Path};

/// universal Result, but not sendable
pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg(feature = "elevation")]
pub mod elevation;
mod utils;
// pub use

pub fn convert_file(fit_path: impl AsRef<Path>) -> Res<()> {
    let fit = Fit::from_file(fit_path)?;
    fit.save_to_gpx()
}
pub fn convert_fit(read: impl std::io::Read, fname: impl AsRef<Path>) -> Res<()> {
    let fit = Fit::from_fit(read)?.with_filename(fname.as_ref().to_str().unwrap());
    fit.save_to_gpx()
}

pub fn write_gpx_to_file(gpx: Gpx, fname: impl AsRef<Path>) -> Res<()> {
    let fpath = Path::new(fname.as_ref());
    // Create file at path
    let gpx_file = File::create(fpath)?;
    let buf = BufWriter::new(gpx_file);

    // Write to file
    gpx::write(&gpx, buf)?;
    Ok(())
}

/// Fit Context structure. An instance of this will be passed to the parser and ultimately to the callback function so we can use it for whatever.
#[derive(Default, Clone)]
pub struct Fit {
    file_name: String,
    sum00: u32,
    num_records_processed: u16,
    pub track_segment: TrackSegment,
}
impl Fit {
    /// no need to clone the whole [`Fit`], only the `file_name`: a [`String`]
    pub fn file_name(&self) -> String {
        self.file_name.to_owned()
    }
    /// add a filename to `self`, create new instance
    pub fn with_filename(self, fname: impl Into<String>) -> Self {
        Fit {
            file_name: fname.into(),
            ..self
        }
    }
    /// create a [`Fit`] from a `path`, where a fit file lies
    // TODO: docs
    pub fn from_file(fit_path: impl AsRef<Path>) -> Res<Self> {
        let file = std::fs::File::open(&fit_path)?;
        let mut bufread = std::io::BufReader::new(file);

        Ok(Self::from_fit(&mut bufread)?.with_filename(fit_path.as_ref().to_str().unwrap()))
    }

    /// Called for each record message as it is being processed.
    // TODO: don't panic
    pub fn callback(
        timestamp: u32,
        global_message_num: u16,
        _local_msg_type: u8,
        _message_index: u16,
        fields: Vec<FitFieldValue>,
        data: &mut Fit,
    ) {
        if global_message_num == fit_file::GLOBAL_MSG_NUM_SESSION {
            let msg = FitSessionMsg::new(fields);
            let sport_names = fit_file::init_sport_name_map();
            let sport_id = msg.sport.unwrap();

            println!("Sport: {}", sport_names.get(&sport_id).unwrap());
        } else if global_message_num == fit_file::GLOBAL_MSG_NUM_RECORD {
            let mut msg = FitRecordMsg::new(fields);

            data.num_records_processed += 1;

            if let Some(ts) = msg.timestamp {
                assert_eq!(timestamp, ts);
            } else {
                msg.timestamp = Some(timestamp);
            }

            if utils::no_lat_lon(&msg) {
                data.sum00 += 1;
            }

            let wp = frm_to_gwp(msg);
            data.track_segment.points.push(wp);
        }
    }

    // TODO: docs
    pub fn from_fit(reader: impl std::io::Read) -> Res<Self> {
        let mut fit = Fit::default();

        let mut bufread = std::io::BufReader::new(reader);
        fit_file::read(&mut bufread, Self::callback, &mut fit)?;

        let percent_00 = fit.sum00 as f32 / fit.track_segment.points.len() as f32;
        let no_00_remains = fit.sum00 > 0 && percent_00 < 0.9;
        if no_00_remains {
            eprintln!("less than 90% ({} out of {} = {percent_00}) doesn't contain latitude and longitude => deleting these points",
             fit.sum00, fit.track_segment.points.len());
        }
        fit.track_segment.points.retain(|wp| {
            let (x, y) = wp.point().x_y();
            (!no_00_remains || !is_00(wp))
                && (-90. ..90.).contains(&y)
                && (-180. ..180.).contains(&x)
        });
        Ok(fit)
    }
    pub fn save_to_gpx(self) -> Res<()> {
        let fname = self.file_name();
        let gpx: Gpx = self.into();
        write_gpx_to_file(gpx, &fname)
    }

    #[cfg(feature = "elevation")]
    /// add elevation data to the `fit` file, using srtm data from `elev_data_dir`
    pub fn add_elev(fit: &mut Fit, elev_data_dir: Option<impl AsRef<Path>>) {
        use elevation::*;
        let needed_tile_coords = needed_tile_coords(&fit.track_segment.points);
        let needed_tiles = read_needed_tiles(&needed_tile_coords, elev_data_dir);
        let all_elev_data = get_all_elev_data(&needed_tile_coords, &needed_tiles);

        add_elev_unchecked(&mut fit.track_segment.points, &all_elev_data);
    }
}
impl From<Fit> for Gpx {
    fn from(fit: Fit) -> Self {
        // Instantiate Gpx struct
        let track = Track {
            segments: vec![fit.track_segment],
            ..Track::default()
        };
        Self {
            version: GpxVersion::Gpx11,
            tracks: vec![track],
            ..Self::default()
        }
    }
}
