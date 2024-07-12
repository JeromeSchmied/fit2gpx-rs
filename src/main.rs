use clap::Parser;
use fit_file as fit;
use fit_file::{fit_file, FitFieldValue, FitRecordMsg, FitSessionMsg};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
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
#[derive(Default)]
struct Context {
    sum00: u32,
    num_records_processed: u16,
    track_segment: TrackSegment,
}

fn fit2gpx(f_in: &str, config: &Args) -> Res<()> {
    let file = std::fs::File::open(f_in)?;

    let mut reader = std::io::BufReader::new(file);
    let mut cx = Context::default();
    fit_file::read(&mut reader, callback, &mut cx)?;

    let percent_00 = cx.sum00 as f32 / cx.track_segment.points.len() as f32;
    let no_00_remains = cx.sum00 > 0 && percent_00 < 0.9;
    if no_00_remains {
        eprintln!("less than 90% ({} out of {} = {}) doesn't contain latitude and longitude => deleting these points",
             cx.sum00, cx.track_segment.points.len(), percent_00);
    }
    cx.track_segment.points.retain(|wp| {
        let (x, y) = wp.point().x_y();
        (if no_00_remains { !is_00(wp) } else { true })
            && (-90. ..90.).contains(&y)
            && (-180. ..180.).contains(&x)
    });
    if config.add_altitude {
        add_altitude(&mut cx.track_segment.points, &config.elev_data_dir);

        // coordinate_altitude::add_altitude(&mut coords)?;

        // for (i, point) in cx.track_segment.points.iter_mut().enumerate() {
        //     if point.elevation.is_none() {
        //         point.elevation = coords.get(i).map(|c| c.altitude);
        //     }
        // }
    }

    // Instantiate Gpx struct
    let track = Track {
        segments: vec![cx.track_segment],
        ..Track::default()
    };
    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        tracks: vec![track],
        ..Gpx::default()
    };

    let f_out = f_in.replace(".fit", ".gpx");
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

fn add_altitude(wps: &mut [Waypoint], elev_data_dir: &Option<PathBuf>) {
    // coord is x,y but we need y,x
    let xy_yx = |wp: &Waypoint| -> srtm::Coord {
        let (x, y) = wp.point().x_y();
        (y, x).into()
    };
    // kinda Waypoint to (i32, i32)
    let trunc = |wp: &Waypoint| -> (i32, i32) {
        let (x, y) = wp.point().x_y();
        (y.trunc() as i32, x.trunc() as i32)
    };
    // tiles we need
    let mut needs: Vec<srtm::Coord> = Vec::new();
    for wp in wps.iter().filter(|wp| !is_00(wp)).map(trunc) {
        if !needs.contains(&wp.into()) {
            needs.push(wp.into());
        }
    }
    if needs.is_empty() {
        return;
    }

    let elev_data_dir = if let Some(arg_data_dir) = &elev_data_dir {
        arg_data_dir.into()
    } else if let Some(env_data_dir) = option_env!("elev_data_dir") {
        PathBuf::from(env_data_dir)
    } else {
        panic!("no elevation data dir is passed as an arg or set as an environment variable: elev_data_dir");
    };
    let tiles = needs
        .iter()
        .map(|c| srtm::get_filename(*c))
        .map(|t| elev_data_dir.join(t))
        .map(|p| srtm::Tile::from_file(p).unwrap())
        .collect::<Vec<_>>();
    let all_elev_data = needs
        .iter()
        .enumerate()
        .map(|(i, coord)| (coord.trunc(), tiles.get(i).unwrap()))
        .collect::<HashMap<_, _>>();
    eprintln!("loaded elevation data: {:?}", all_elev_data.keys());
    for wp in wps
        .iter_mut()
        .filter(|wp| wp.elevation.is_none() && !is_00(wp))
    {
        let coord: srtm::Coord = xy_yx(wp);
        let elev_data = all_elev_data.get(&coord.trunc()).unwrap();
        wp.elevation = Some(elev_data.get(coord) as f64);
    }
}

fn main() {
    // collecting cli args
    let args = Args::parse();

    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();
    for file in args.files.iter() {
        if !file.ends_with(".fit") {
            eprintln!("invalid file: {file:?}");
            continue;
        }
        if !args.overwrite {
            let as_gpx = PathBuf::from(&file.clone().replace(".fit", ".gpx"));
            if as_gpx.exists() {
                continue;
            }
        }
        let file = file.clone();
        let args = args.clone();
        let jh = std::thread::spawn(move || {
            let _ = fit2gpx(&file, &args).inspect_err(|e| eprintln!("error: {e:#?}"));
        });
        jh.join().unwrap();
        // handles.push(jh);
    }
    for handle in handles {
        let res = handle.join();
        if let Err(e) = res {
            eprintln!("error: {e:#?}");
            continue;
        }
    }
}
