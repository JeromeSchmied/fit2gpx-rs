use super::*;
use geo_types::{coord, Point};
use time::OffsetDateTime;

/// [`fit_file::FitRecordMsg`] to gpx Waypoint
// TODO: support heart-rate, distance, temperature and such extensions, if `gpx` crate does too
pub(crate) fn frm_to_gwp(frm: FitRecordMsg) -> Waypoint {
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

// TODO: docs
pub(crate) fn no_lat_lon(frm: &FitRecordMsg) -> bool {
    frm.position_long.is_none() && frm.position_lat.is_none()
}
// TODO: docs
pub(crate) fn is_00(wp: &Waypoint) -> bool {
    wp.point().x_y() == (0., 0.)
}
