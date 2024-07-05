use clap::Parser;
use fit_file as fit;
use fit_file::{fit_file, FitFieldValue, FitRecordMsg, FitSessionMsg};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::{fs::File, io::BufWriter};
use time::OffsetDateTime;

/// universal Result
type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    pub files: Vec<String>,
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
            data.no_lat_lon_sum += 1;
        }

        let wp = frm_to_gwp(msg);
        data.track_segment.points.push(wp);
    }
}

/// Context structure. An instance of this will be passed to the parser and ultimately to the callback function so we can use it for whatever.
#[derive(Default)]
struct Context {
    no_lat_lon_sum: u32,
    num_records_processed: u16,
    track_segment: TrackSegment,
}

fn fit2gpx(f_in: &str) -> Res<()> {
    let file = std::fs::File::open(f_in)?;

    let mut reader = std::io::BufReader::new(file);
    let mut cx = Context::default();
    fit_file::read(&mut reader, callback, &mut cx)?;

    let percent_no_lat_lon = cx.no_lat_lon_sum as f32 / cx.track_segment.points.len() as f32;
    if cx.no_lat_lon_sum > 0 && percent_no_lat_lon < 0.9 {
        eprintln!("less than 90% ({} out of {} = {}) doesn't contain latitude and longitude => deleting these points", cx.no_lat_lon_sum, cx.track_segment.points.len(), percent_no_lat_lon);
        cx.track_segment
            .points
            .retain(|point| point.point().x_y() != (0., 0.));
    }

    // Instantiate Gpx struct
    let track = Track {
        name: None,
        comment: None,
        description: None,
        source: None,
        links: vec![],
        type_: None,
        number: None,
        segments: vec![cx.track_segment],
    };
    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        creator: None,
        metadata: None,
        waypoints: vec![],
        tracks: vec![track],
        routes: vec![],
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

fn main() {
    // collecting cli args
    let args = Args::parse();

    let mut handles = vec![];
    for file in args.files.iter() {
        if !file.ends_with(".fit") {
            eprintln!("invalid file: {file:?}");
            continue;
        }
        let file = file.clone();
        let jh = std::thread::spawn(move || {
            let _ = fit2gpx(&file).inspect_err(|e| eprintln!("error: {e:#?}"));
        });
        handles.push(jh);
    }
    for handle in handles {
        let res = handle.join();
        if let Err(e) = res {
            eprintln!("error: {e:#?}");
            continue;
        }
    }
}
