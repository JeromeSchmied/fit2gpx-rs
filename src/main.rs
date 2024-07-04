use fit_rust::{
    protocol::{
        data_field::DataField, message_type::MessageType, value::Value, FitDataMessage, FitMessage,
    },
    Fit,
};
use std::fs;

fn main() {
    // collecting cli args
    let args = std::env::args().collect::<Vec<_>>();
    let file_path = args.get(1).unwrap_or_else(|| {
        println!("no file path specified");
        std::process::exit(1)
    });

    let file = fs::read(file_path).unwrap();
    let fit: Fit = Fit::read(file).unwrap();
    println!("\n\nHEADER:");
    println!("\theader size: {}", &fit.header.header_size);
    println!("\tprotocol version: {}", &fit.header.protocol_version);
    println!("\tprofile version: {}", &fit.header.profile_version);
    println!("\tdata_size: {}", &fit.header.data_size);
    println!("\tdata_type: {}", &fit.header.data_type);
    println!("\tcrc: {:?}", &fit.header.crc);
    println!("-----------------------------\n");

    for data in &fit.data {
        match data {
            FitMessage::Definition(_msg) => {
                // println!("\nDefinition: {:#?}", msg.data);
            }
            FitMessage::Data(msg) => {
                // println!("\nData: {:#?}", msg.data);
                if let MessageType::Record = msg.data.message_type {
                    let x: f32 = match df_at(msg, 0) {
                        Value::F32(x) => *x,
                        x => panic!("invalid x coordinate: {x:?}"),
                    };
                    let y: f32 = match df_at(msg, 1) {
                        Value::F32(y) => *y,
                        y => panic!("invalid y coordinate: {y:?}"),
                    };

                    let t = match df_at(msg, 253) {
                        Value::Time(t) => t,
                        t => panic!("invalid time: {t:?}"),
                    };

                    println!("at {} at ({};{})", t, x, y);
                }
            }
        }
    }
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
