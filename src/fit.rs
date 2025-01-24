use crate::{utils, Res};
use fit_file::{fit_file, FitFieldValue, FitRecordMsg};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

/// Fit Context structure. An instance of this will be passed to the parser and ultimately to the callback function so we can use it for whatever.
#[derive(Default, Clone)]
pub struct Fit {
    pub file_name: PathBuf,
    num_records_processed: u16,
    pub track_segment: TrackSegment,
}

// high level functions impl
impl Fit {
    /// add a filename to `self`, create new instance
    pub fn with_filename(self, fname: impl Into<PathBuf>) -> Self {
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

        Ok(Self::from_reader(&mut bufread)?.with_filename(fit_path.as_ref()))
    }

    // TODO: docs
    pub fn from_reader(reader: impl std::io::Read) -> Res<Self> {
        let mut fit = Fit::default();

        let mut bufread = std::io::BufReader::new(reader);
        fit_file::read(&mut bufread, Self::callback, &mut fit)?;

        fit.track_segment.points.retain(|wp| {
            let (x, y) = wp.point().x_y();
            !utils::is_00(wp) && (-90. ..90.).contains(&y) && (-180. ..180.).contains(&x)
        });
        Ok(fit)
    }
    pub fn file_to_gpx(fit_path: impl AsRef<Path>) -> Res<()> {
        let fit = Fit::from_file(fit_path)?;
        fit.save_to_gpx()
    }

    pub fn reader_to_gpx(read: impl std::io::Read, fname: impl AsRef<Path>) -> Res<()> {
        let fit = Fit::from_reader(read)?.with_filename(fname.as_ref());
        fit.save_to_gpx()
    }

    pub fn save_to_gpx(self) -> Res<()> {
        let fname = self.file_name.with_extension("gpx");
        let gpx: Gpx = self.into();
        utils::write_gpx_to_file(gpx, &fname)
    }

    #[cfg(feature = "elevation")]
    /// add elevation data to the `fit` file, using srtm data from `elev_data_dir`
    pub fn add_elev(fit: &mut Fit, elev_data_dir: impl AsRef<Path>, overwrite: bool) {
        use super::elevation::*;
        let needed_tile_coords = needed_tile_coords(&fit.track_segment.points);
        let needed_tiles = read_needed_tiles(&needed_tile_coords, elev_data_dir);
        let all_elev_data = index_needed_tiles(&needed_tile_coords, &needed_tiles);

        add_elev_unchecked(&mut fit.track_segment.points, &all_elev_data, overwrite);
    }
}
impl Fit {
    /// [`fit_file::FitRecordMsg`] to [`gpx::Waypoint`]
    // TODO: support heart-rate, distance, temperature and such extensions, if `gpx` crate does too
    fn frm_to_gwp(frm: FitRecordMsg) -> Waypoint {
        let time = frm.timestamp.unwrap_or(0);
        let time = OffsetDateTime::from_unix_timestamp(time.into()).ok();

        let lat = fit_file::semicircles_to_degrees(frm.position_lat.unwrap_or(0));
        let lon = fit_file::semicircles_to_degrees(frm.position_long.unwrap_or(0));

        let alt = if let Some(enh_alt) = frm.enhanced_altitude {
            Some(enh_alt)
        } else {
            frm.altitude.map(|alt| alt.into())
        }
        .map(|alt| alt as f32 / 5. - 500.); // m

        // let dist = frm.distance.unwrap_or(0) as f32 / 100000.; // km

        let speed = if let Some(enh_spd) = frm.enhanced_speed {
            Some(enh_spd)
        } else {
            frm.speed.map(|spd| spd.into())
        }
        .map(|spd| spd as f64);
        // .map(|spd| spd as f64 / 1000. * 3.6); // km/h

        // let hr = frm
        //     .heart_rate
        //     .map(|hr| hr.checked_add(1))
        //     .unwrap_or(Some(0))
        //     .unwrap_or(0);

        // let temp = frm.temperature.unwrap_or(i8::MIN);

        let geo_point = Point(coord! {x: lon, y: lat});

        let mut wp = Waypoint::new(geo_point);

        wp.elevation = alt.map(|alt| alt.into());
        wp.time = time.map(|t| t.into());
        wp.speed = speed;

        wp
    }
    /// Called for each record message as it is being processed.
    // TODO: don't panic
    fn callback(
        timestamp: u32,
        global_message_num: u16,
        _local_msg_type: u8,
        _message_index: u16,
        fields: Vec<FitFieldValue>,
        data: &mut Fit,
    ) {
        if global_message_num == fit_file::GLOBAL_MSG_NUM_SESSION {
            // let msg = FitSessionMsg::new(fields);
            // let sport_names = fit_file::init_sport_name_map();
            // let sport_id = msg.sport.unwrap();

            // println!("Sport: {}", sport_names.get(&sport_id).unwrap());
        } else if global_message_num == fit_file::GLOBAL_MSG_NUM_RECORD {
            let mut msg = FitRecordMsg::new(fields);

            data.num_records_processed += 1;

            if let Some(ts) = msg.timestamp {
                assert_eq!(timestamp, ts);
            } else {
                msg.timestamp = Some(timestamp);
            }

            let wp = Self::frm_to_gwp(msg);
            data.track_segment.points.push(wp);
        }
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
