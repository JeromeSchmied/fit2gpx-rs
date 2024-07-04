use fit_rust::{
    protocol::{message_type::MessageType, value::Value, FitDataMessage, FitMessage},
    Fit,
};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::fs;
use std::{fs::File, io::BufWriter};

fn main() {
    // collecting cli args
    let args = std::env::args().collect::<Vec<_>>();
    let f_in = args.get(1).unwrap_or_else(|| {
        println!("no file path specified");
        std::process::exit(1)
    });

    let file = fs::read(f_in).unwrap();
    let fit: Fit = Fit::read(file).unwrap();

    println!("\n\nHEADER:");
    println!("\theader size: {}", &fit.header.header_size);
    println!("\tprotocol version: {}", &fit.header.protocol_version);
    println!("\tprofile version: {}", &fit.header.profile_version);
    println!("\tdata_size: {}", &fit.header.data_size);
    println!("\tdata_type: {}", &fit.header.data_type);
    println!("\tcrc: {:?}", &fit.header.crc);
    println!("-----------------------------\n");

    let mut track_segment = TrackSegment { points: vec![] };

    for data in &fit.data {
        match data {
            FitMessage::Definition(_msg) => {
                // println!("\nDefinition: {:#?}", msg.data);
            }
            FitMessage::Data(msg) => {
                // println!("\nData: {:#?}", msg.data);
                if let MessageType::Record = msg.data.message_type {
                    let y: f32 = match df_at(msg, 0) {
                        Value::F32(y) => *y,
                        y => panic!("invalid y coordinate: {y:?}"),
                    };
                    let x: f32 = match df_at(msg, 1) {
                        Value::F32(x) => *x,
                        x => panic!("invalid x coordinate: {x:?}"),
                    };

                    // let elev = todo!(); TODO

                    let t = match df_at(msg, 253) {
                        Value::Time(t) => t,
                        t => panic!("invalid time: {t:?}"),
                    };
                    let t = time::OffsetDateTime::from_unix_timestamp((*t).into()).unwrap();

                    // Add track point
                    let geo_point: Point = Point(coord! {x: x as f64, y: y as f64});
                    let mut wp = Waypoint::new(geo_point);
                    // wp.elevation = elev; // TODO
                    wp.time = Some(t.into());
                    track_segment.points.push(wp);
                }
            }
        }
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
        segments: vec![track_segment],
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
    let gpx_file = File::create(f_out).unwrap();
    let buf = BufWriter::new(gpx_file);

    // Write to file
    gpx::write(&gpx, buf).unwrap();
}

/// datafield at num
fn df_at(data_msg: &FitDataMessage, num: u8) -> &Value {
    let x = data_msg
        .data
        .values
        .iter()
        .filter(|df| df.field_num == num)
        .collect::<Vec<_>>();
    assert_eq!(1, x.len());

    &x[0].value
}
