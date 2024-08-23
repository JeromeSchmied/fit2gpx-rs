use clap::Parser;
use fit_file as fit;
use fit_file::{fit_file, FitFieldValue, FitRecordMsg, FitSessionMsg};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use rayon::prelude::*;
use std::{collections::HashMap, fs::File, io::BufWriter, path::PathBuf};
use time::OffsetDateTime;

/// universal Result
type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    pub files: Vec<String>,
    #[arg(short = 'd', long)]
    pub elev_data_dir: Option<PathBuf>,
    #[arg(short, long, default_value_t = false)]
    pub add_altitude: bool,
    #[arg(short, long, default_value_t = false)]
    pub overwrite: bool,
}

// FitRecordMsg to gpx Waypoint
fn frm_to_gwp(frm: FitRecordMsg) -> Waypoint {
    let time = frm.timestamp.unwrap_or(0);
    let time = OffsetDateTime::from_unix_timestamp(time.into()).ok();

    let lat = fit::semicircles_to_degrees(frm.position_lat.unwrap_or(0));
    let lon = fit::semicircles_to_degrees(frm.position_long.unwrap_or(0));

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

    let geo_point: Point = Point(coord! {x: lon, y: lat});

    let mut wp = Waypoint::new(geo_point);

    wp.elevation = alt.map(|alt| alt.into());
    wp.time = time.map(|t| t.into());
    wp.speed = speed;

    wp
}

fn no_lat_lon(frm: &FitRecordMsg) -> bool {
    frm.position_long.is_none() && frm.position_lat.is_none()
}

/// Called for each record message as it is processed.
fn callback(
    timestamp: u32,
    global_message_num: u16,
    _local_msg_type: u8,
    _message_index: u16,
    fields: Vec<FitFieldValue>,
    data: &mut Context,
) {
    if global_message_num == fit::GLOBAL_MSG_NUM_SESSION {
        let msg = FitSessionMsg::new(fields);
        let sport_names = fit_file::init_sport_name_map();
        let sport_id = msg.sport.unwrap();

        println!("Sport: {}", sport_names.get(&sport_id).unwrap());
    } else if global_message_num == fit::GLOBAL_MSG_NUM_RECORD {
        let mut msg = FitRecordMsg::new(fields);

        data.num_records_processed += 1;

        if let Some(ts) = msg.timestamp {
            assert_eq!(timestamp, ts);
        } else {
            msg.timestamp = Some(timestamp);
        }

        if no_lat_lon(&msg) {
            data.sum00 += 1;
        }

        let wp = frm_to_gwp(msg);
        data.track_segment.points.push(wp);
    }
}

/// Context structure. An instance of this will be passed to the parser and ultimately to the callback function so we can use it for whatever.
#[derive(Default, Clone)]
struct Context {
    file_name: String,
    sum00: u32,
    num_records_processed: u16,
    track_segment: TrackSegment,
}

fn read_fit(fit: &str) -> Res<Context> {
    let mut cx = Context {
        file_name: fit.to_owned(),
        ..Context::default()
    };
    let file = std::fs::File::open(fit)?;

    let mut reader = std::io::BufReader::new(file);
    // let mut cx = Context::default();
    fit_file::read(&mut reader, callback, &mut cx)?;

    let percent_00 = cx.sum00 as f32 / cx.track_segment.points.len() as f32;
    let no_00_remains = cx.sum00 > 0 && percent_00 < 0.9;
    if no_00_remains {
        eprintln!("less than 90% ({} out of {} = {}) doesn't contain latitude and longitude => deleting these points",
             cx.sum00, cx.track_segment.points.len(), percent_00);
    }
    cx.track_segment.points.retain(|wp| {
        let (x, y) = wp.point().x_y();
        (!no_00_remains || !is_00(wp)) && (-90. ..90.).contains(&y) && (-180. ..180.).contains(&x)
    });
    Ok(cx)
}
fn fit2gpx(cx: Context) -> Res<()> {
    // Instantiate Gpx struct
    let track = Track {
        segments: vec![cx.track_segment.clone()],
        ..Track::default()
    };
    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        tracks: vec![track],
        ..Gpx::default()
    };

    let f_out = cx.file_name.replace(".fit", ".gpx");
    // Create file at path
    let gpx_file = File::create(f_out)?;
    let buf = BufWriter::new(gpx_file);

    // Write to file
    gpx::write(&gpx, buf)?;
    println!("{} records processed", cx.num_records_processed);
    Ok(())
}
fn is_00(wp: &Waypoint) -> bool {
    wp.point().x_y() == (0., 0.)
}

fn needed_tile_coords(wps: &[Waypoint]) -> Vec<(i32, i32)> {
    // kinda Waypoint to (i32, i32)
    let trunc = |wp: &Waypoint| -> (i32, i32) {
        let (x, y) = wp.point().x_y();
        (y.trunc() as i32, x.trunc() as i32)
    };
    // tiles we need
    let mut needs = Vec::new();
    for wp in wps.iter().filter(|wp| !is_00(wp)).map(trunc) {
        if !needs.contains(&wp) {
            needs.push(wp);
        }
    }
    needs
}

fn needed_tiles(needs: &[(i32, i32)], elev_data_dir: &Option<PathBuf>) -> Vec<srtm::Tile> {
    if needs.is_empty() {
        return vec![];
    }

    let elev_data_dir = if let Some(arg_data_dir) = &elev_data_dir {
        arg_data_dir.into()
    } else if let Some(env_data_dir) = option_env!("elev_data_dir") {
        PathBuf::from(env_data_dir)
    } else {
        panic!("no elevation data dir is passed as an arg or set as an environment variable: elev_data_dir");
    };
    needs
        .par_iter()
        .map(|c| srtm::get_filename(*c))
        .map(|t| elev_data_dir.join(t))
        .map(|p| srtm::Tile::from_file(p).unwrap())
        .collect::<Vec<_>>()
}
fn get_all_elev_data<'a>(
    needs: &'a [(i32, i32)],
    tiles: &'a [srtm::Tile],
) -> HashMap<&'a (i32, i32), &'a srtm::Tile> {
    assert_eq!(needs.len(), tiles.len());
    if needs.is_empty() {
        return HashMap::new();
    }
    let all_elev_data = needs
        .par_iter()
        .enumerate()
        .map(|(i, coord)| (coord, tiles.get(i).unwrap()))
        .collect::<HashMap<_, _>>();
    eprintln!("loaded elevation data: {:?}", all_elev_data.keys());
    all_elev_data
}

fn add_elev(wps: &mut [Waypoint], elev_data: &HashMap<&(i32, i32), &srtm::Tile>) -> Res<()> {
    // coord is x,y but we need y,x
    let xy_yx = |wp: &Waypoint| -> srtm::Coord {
        let (x, y) = wp.point().x_y();
        (y, x).into()
    };
    wps.par_iter_mut()
        .filter(|wp| wp.elevation.is_none() && !is_00(wp))
        .for_each(|wp| {
            let coord = xy_yx(wp);
            let elev_data = elev_data.get(&coord.trunc()).unwrap();
            wp.elevation = Some(elev_data.get(coord) as f64);
        });
    Ok(())
}

fn main() -> Res<()> {
    // collecting cli args
    let conf = Args::parse();

    let all_fit = conf
        .files
        .par_iter()
        .filter(|f| {
            f.ends_with(".fit")
                && (conf.overwrite || !PathBuf::from(f.replace(".fit", ".gpx")).exists())
        })
        .flat_map(|f| read_fit(f).inspect_err(|e| eprintln!("read error: {e:?}")))
        .collect::<Vec<_>>();

    let all_needed_tile_coords = if conf.add_altitude {
        let mut all = all_fit
            .par_iter()
            .flat_map(|cx| needed_tile_coords(&cx.track_segment.points))
            .collect::<Vec<_>>();
        all.sort_unstable();
        all.dedup();

        all
    } else {
        vec![]
    };
    let all_needed_tiles = needed_tiles(&all_needed_tile_coords, &conf.elev_data_dir);
    let all_elev_data = get_all_elev_data(&all_needed_tile_coords, &all_needed_tiles);
    // coordinate_altitude::add_altitude(&mut coords)?;
    // for (i, point) in cx.track_segment.points.iter_mut().enumerate() {
    //     if point.elevation.is_none() {
    //         point.elevation = coords.get(i).map(|c| c.altitude);
    //     }
    // }

    all_fit
        .into_iter()
        .try_for_each(|mut cx: Context| -> Res<()> {
            if conf.add_altitude {
                let _ = add_elev(&mut cx.track_segment.points, &all_elev_data)
                    .inspect_err(|e| eprintln!("elevation error: {e:?}"));
            }
            fit2gpx(cx).inspect_err(|e| eprintln!("convertion error: {e:?}"))
        })?;

    Ok(())
}
