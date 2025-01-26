use fit2gpx::Fit;

fn main() {
    // read into memory
    let mut fit = Fit::from_file("rundumadum.fit").unwrap();
    // add elevation, requires `./N48E016.hgt`
    // fit.add_elev_read(".", false).unwrap();
    // silly modification
    fit.track_segment.points.iter_mut().for_each(|p| {
        if let Some(ele) = &mut p.elevation {
            *ele += 3000.;
        } else {
            p.elevation = Some(4000.);
        }
    });

    let out_path = std::path::PathBuf::from("rundumadum_elevation.gpx");
    fit.save_to_gpx(&out_path).unwrap();
    assert!(out_path.exists());
    // cleanup
    std::fs::remove_file(out_path).unwrap();
}
