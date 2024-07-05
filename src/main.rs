use clap::Parser;
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::{fs::File, io::BufWriter};
use time::OffsetDateTime;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    pub files: Vec<String>,
}

// FitRecordMsg to gpx Waypoint
fn frm_to_gwp(frm: FitRecordMsg) -> Waypoint {
    // "Time: {}\tLat: {}\tLon: {}\tAlt: {:.1}m\tDist: {:.3}km\tSpeed: {:.1}km/h\tHR: {}",
    // eprintln!("time: {:?}", frm.timestamp);
    let time = frm.timestamp.unwrap_or(0);
    let time = OffsetDateTime::from_unix_timestamp(time.into()).ok();

    let lat = fit_file::semicircles_to_degrees(frm.position_lat.unwrap_or(0));
    let lon = fit_file::semicircles_to_degrees(frm.position_long.unwrap_or(0));

    let alt = if let Some(enh_alt) = frm.enhanced_altitude {
        Some(enh_alt)
    } else {
        frm.altitude.map(|alt| alt.into())
    }
    .map(|alt| alt as f32 / 5. - 500.);

    // let dist = frm.distance.unwrap_or(0) as f32 / 100000.;

    let speed = if let Some(enh_spd) = frm.enhanced_speed {
        Some(enh_spd)
    } else {
        frm.speed.map(|spd| spd.into())
    }
    .map(|spd| spd as f64);
    // .map(|spd| spd as f64 / 1000. * 3.6);

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

use fit_file::{fit_file, FitRecordMsg, FitSessionMsg};

/// Called for each record message as it is processed.
fn callback(
    timestamp: u32,
    global_message_num: u16,
    _local_msg_type: u8,
    _message_index: u16,
    fields: Vec<fit_file::FitFieldValue>,
    data: &mut Context,
) {
    if global_message_num == fit_file::GLOBAL_MSG_NUM_DEVICE_INFO {
        // let msg = FitDeviceInfoMsg::new(fields);
        // println!("{msg:#?}");
    } else if global_message_num == fit_file::GLOBAL_MSG_NUM_SESSION {
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

        if no_lat_lon(&msg) {
            data.no_lat_lon_sum += 1;
        }

        // println!(
        //     "timestamp: {:?}|{:?}|{:?}|{}",
        //     msg.timestamp, msg.time128, msg.time_from_course, timestamp
        // );
        let wp = frm_to_gwp(msg);
        data.track_segment.points.push(wp);
        // assert!(msg.timestamp.is_some());

        // println!("{msg:#?}");

        // println!(
        //     // "Timestamp: {} Latitude: {} Longitude: {} Altitude: {} Distance: {} Speed: {} HeartRate: {}",
        //     "Time: {}\tLat: {}\tLon: {}\tAlt: {:.1}m\tDist: {:.3}km\tSpeed: {:.1}km/h\tHR: {}",
        //     msg.timestamp.unwrap_or(0),
        //     fit_file::semicircles_to_degrees(msg.position_lat.unwrap_or(0)),
        //     fit_file::semicircles_to_degrees(msg.position_long.unwrap_or(0)),
        //     msg.enhanced_altitude.unwrap_or(0) as f32 / 5. - 500.,
        //     msg.distance.unwrap_or(0) as f32 / 100000.,
        //     msg.enhanced_speed.unwrap_or(0) as f32 / 1000. * 3.6,
        //     msg.heart_rate
        //         .map(|hr| hr.checked_add(1))
        //         .unwrap_or(Some(0))
        //         .unwrap_or(0),
        // );
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

//                 if let MessageType::Record = msg.data.message_type {
//                     let rec_dat: RecordData = msg.data.clone().into();
//                     if rec_dat.no_lat_lon() {
//                         no_lat_lon_sum += 1;
//                         // eprintln!("doesn't contain lat/lon data: {:?}", msg.data);
//                         // continue;
//                     }
//                     // eprintln!("{rec_dat:#?}");
//                     // Add track point
//                     let wp: Waypoint = rec_dat.into();
//                     // if wp.point().x_y() == (0., 0.) {
//                     //     eprintln!("warn: guess it's invalid: {msg:#?}");
//                     // }
//                     if ongoing_activity {
//                         track_segment.points.push(wp);
//                     } else {
//                         eprintln!("warn: NOT in an activity right now");
//                         // std::io::stdin().read_line(&mut String::new())?;
//                     }
//                 } else if let MessageType::Activity = msg.data.message_type {
//                     let start_stop = df_at(&msg.data, 4);
//                     if let Some(Value::Enum(start_stop)) = start_stop {
//                         if start_stop == &"start" {
//                             ongoing_activity = true;
//                         } else if start_stop == &"stop" {
//                             ongoing_activity = false;

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
