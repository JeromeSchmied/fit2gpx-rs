use clap::Parser;
use fit_rust::{
    protocol::{message_type::MessageType, value::Value, DataMessage, FitMessage},
    Fit,
};
use geo_types::{coord, Point};
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use std::{fs, fs::File, io::BufWriter};
use time::OffsetDateTime;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    pub files: Vec<String>,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct RecordData {
    /// latitude
    pub lat: Option<f64>,
    /// longitude
    pub lon: Option<f64>,
    /// altitude
    pub alt: Option<f64>,
    // heart-rate
    pub hr: Option<u8>,
    /// timestamp
    pub time: Option<OffsetDateTime>,

    pub cadence: Option<u8>,
    pub distance: Option<f64>,
    pub speed: Option<f64>,
    pub power: Option<u16>,
    pub temperature: Option<i8>,
    pub right_balance: Option<u8>,
}
impl RecordData {
    // crazy check
    fn invalid(&self) -> bool {
        self.lat.is_none() && self.lon.is_none()
    }
}
impl From<DataMessage> for RecordData {
    fn from(value: DataMessage) -> Self {
        if let MessageType::Record = value.message_type {
            let lat = value_to_float(df_at(&value, 0));
            let lon = value_to_float(df_at(&value, 1));
            let alt = value_to_float(df_at(&value, 2)).map(|alt| alt / 5. - 500.);

            let hr = value_to_float(df_at(&value, 3));
            let hr: Option<u8> = hr.map(|hr| hr as u8);

            let cadence = value_to_float(df_at(&value, 4));
            let cadence: Option<u8> = cadence.map(|cad| cad as u8);

            let distance = value_to_float(df_at(&value, 5)).map(|d| d / 100000.);

            let speed = value_to_float(df_at(&value, 6)).map(|v| v / 1000. * 3.6);

            let power = value_to_float(df_at(&value, 7));
            let power = power.map(|power| power as u16);

            let temperature = value_to_float(df_at(&value, 13));
            let temperature = temperature.map(|temperature| temperature as i8);

            let right_balance = value_to_float(df_at(&value, 30));
            let right_balance = right_balance.map(|right_balance| right_balance as u8);

            let t = df_at(&value, 253);
            let time = if let Some(Value::Time(t)) = t {
                if let Ok(t) = OffsetDateTime::from_unix_timestamp((*t).into()) {
                    Some(t)
                } else {
                    None
                }
            } else {
                None
            };

            RecordData {
                lat,
                lon,
                alt,
                hr,
                time,
                cadence,
                distance,
                speed,
                power,
                temperature,
                right_balance,
            }
        } else {
            RecordData::default()
        }
    }
}
impl From<RecordData> for Waypoint {
    fn from(value: RecordData) -> Self {
        let geo_point: Point =
            Point(coord! {x: value.lon.unwrap_or(0.), y: value.lat.unwrap_or(0.)});

        let mut wp = Waypoint::new(geo_point);
        wp.elevation = value.alt;
        wp.time = value.time.map(|t| t.into());
        wp.speed = value.speed;

        wp
    }
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

fn fit2gpx(f_in: &String) -> Res<()> {
    let file = fs::read(f_in)?;
    let fit: Fit = Fit::read(file)?;
    eprintln!("file: {f_in}");
    // let mut log_file = File::create([f_in, ".log"].concat())?;

    // println!("\n\nHEADER:");
    // println!("\theader size: {}", &fit.header.header_size);
    // println!("\tprotocol version: {}", &fit.header.protocol_version);
    // println!("\tprofile version: {}", &fit.header.profile_version);
    // println!("\tdata_size: {}", &fit.header.data_size);
    // println!("\tdata_type: {}", &fit.header.data_type);
    // println!("\tcrc: {:?}", &fit.header.crc);
    // println!("-----------------------------\n");

    let mut track_segment = TrackSegment { points: vec![] };
    let mut ongoing_activity = true;

    for data in &fit.data {
        match data {
            FitMessage::Definition(_msg) => {
                // println!("\nDefinition: {:#?}", msg.data);
            }
            FitMessage::Data(msg) => {
                // writeln!(log_file, "\nData: {msg:#?}")?;
                // println!("\nData: {:#?}", msg);
                if let MessageType::Record = msg.data.message_type {
                    let rec_dat: RecordData = msg.data.clone().into();
                    if rec_dat.invalid() {
                        eprintln!("doesn't contain lat/lon data: {:?}", msg.data);
                        continue;
                    }
                    // eprintln!("{rec_dat:#?}");

                    // Add track point
                    let wp: Waypoint = rec_dat.into();
                    // if wp.point().x_y() == (0., 0.) {
                    //     eprintln!("warn: guess it's invalid: {msg:#?}");
                    // }
                    if ongoing_activity {
                        track_segment.points.push(wp);
                    } else {
                        eprintln!("warn: NOT in an activity right now");
                        // std::io::stdin().read_line(&mut String::new())?;
                    }
                } else if let MessageType::Activity = msg.data.message_type {
                    let start_stop = df_at(&msg.data, 4);
                    if let Some(Value::Enum(start_stop)) = start_stop {
                        if start_stop == &"start" {
                            ongoing_activity = true;
                        } else if start_stop == &"stop" {
                            ongoing_activity = false;
                        }
                    }
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
    let gpx_file = File::create(f_out)?;
    let buf = BufWriter::new(gpx_file);

    // Write to file
    gpx::write(&gpx, buf)?;

    Ok(())
}

fn value_to_float(val: Option<&Value>) -> Option<f64> {
    match val {
        Some(Value::U8(x)) => Some(*x as f64),
        Some(Value::U16(x)) => Some(*x as f64),
        Some(Value::U32(x)) => Some(*x as f64),
        Some(Value::U64(x)) => Some(*x as f64),

        Some(Value::I8(x)) => Some(*x as f64),
        Some(Value::I16(x)) => Some(*x as f64),
        Some(Value::I32(x)) => Some(*x as f64),
        Some(Value::I64(x)) => Some(*x as f64),

        Some(Value::F32(x)) => Some(*x as f64),
        Some(Value::F64(x)) => Some(*x),
        _x => {
            // eprintln!("invalid f64: {x:?}");
            None
        }
    }
}

/// datafield at num
fn df_at(data_msg: &DataMessage, num: u8) -> Option<&Value> {
    // eprintln!("data-msg: {data_msg:#?}");
    let x = data_msg
        .values
        .iter()
        .filter(|df| df.field_num == num)
        .collect::<Vec<_>>();

    Some(&x.first()?.value)
}
