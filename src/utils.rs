use gpx::{Gpx, Waypoint};
use std::{fs::File, io::BufWriter, path::Path};

use crate::Res;

pub fn write_gpx_to_file(gpx: Gpx, fname: impl AsRef<Path>) -> Res<()> {
    // Create file at `fname`
    let gpx_file = File::create(fname.as_ref())?;
    let buf = BufWriter::new(gpx_file);

    // Write to file
    gpx::write(&gpx, buf)?;
    log::debug!("written {:?}", fname.as_ref());
    Ok(())
}
/// is this `wp` null(0; 0)?
pub fn is_00(wp: &Waypoint) -> bool {
    let res = wp.point().x_y() == (0., 0.);
    if res {
        log::trace!("{wp:?} is null");
    }
    res
}
