use crate::utils;
use gpx::Waypoint;
use rayon::prelude::*;
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

// TODO: docs
pub fn needed_tile_coords(wps: &[Waypoint]) -> BTreeSet<(i32, i32)> {
    // kinda Waypoint to (i32, i32)
    let trunc = |wp: &Waypoint| -> (i32, i32) {
        let (x, y) = wp.point().x_y();
        (y.trunc() as i32, x.trunc() as i32)
    };
    // tiles we need
    wps.par_iter()
        .filter(|wp| !utils::is_00(wp))
        .map(trunc)
        .collect()
}

// TODO: docs
pub fn read_needed_tiles(
    needs: &BTreeSet<(i32, i32)>,
    elev_data_dir: impl AsRef<Path>,
) -> Vec<srtm_reader::Tile> {
    log::info!("reading needed tiles into memory");
    log::debug!("needed tiles' coordinates are: {needs:?}");
    if needs.is_empty() {
        return vec![];
    }

    let elev_data_dir = elev_data_dir.as_ref();
    needs
        .par_iter()
        .map(|c| srtm_reader::get_filename(*c))
        .map(|t| elev_data_dir.join(t))
        .flat_map(|p| {
            srtm_reader::Tile::from_file(&p)
                .inspect_err(|e| log::error!("error while reading {p:?} into memory: {e:#?}"))
        })
        .collect::<Vec<_>>()
}
// TODO: docs
/// index the tiles with their coordinates
pub fn index_tiles(tiles: &[srtm_reader::Tile]) -> HashMap<(i32, i32), &srtm_reader::Tile> {
    log::info!("reading all coordinates' elevation into memory");
    log::trace!("elevation needed for coordinates: {tiles:?}");
    tiles
        .par_iter()
        .map(|tile| ((tile.latitude, tile.longitude), tile))
        .collect()
    // log::debug!("loaded elevation data: {:?}", all_elev_data.keys());
}

/// add elevation to all `wps` using `elev_data` if available, in parallel
///
/// # Safety
///
/// it's the caller's responsibility to have the necessary data loaded
///
/// # Usage
///
/// using the following order, it should be safe
///
/// ```no_run
/// use fit2gpx::elevation;
///
/// let mut fit = fit2gpx::Fit::from_file("evening walk.gpx").unwrap();
/// let elev_data_dir = "~/Downloads/srtm_data";
/// let needed_tile_coords = elevation::needed_tile_coords(&fit.track_segment.points);
/// let needed_tiles = elevation::read_needed_tiles(&needed_tile_coords, elev_data_dir);
/// let all_elev_data = elevation::index_needed_tiles(&needed_tiles);
///
/// elevation::add_elev_unchecked(&mut fit.track_segment.points, &all_elev_data, false);
/// ```
pub fn add_elev_unchecked(
    wps: &mut [Waypoint],
    elev_data: &HashMap<(i32, i32), &srtm_reader::Tile>,
    overwrite: bool,
) {
    // coord is (x;y) but we need (y;x)
    let xy_yx = |wp: &Waypoint| -> srtm_reader::Coord {
        let (x, y) = wp.point().x_y();
        (y, x).into()
    };
    wps.into_par_iter()
        .filter(|wp| (wp.elevation.is_none() || overwrite) && !utils::is_00(wp))
        .for_each(|wp| {
            let coord = xy_yx(wp);
            if let Some(elev_data) = elev_data.get(&coord.trunc()) {
                let elev = elev_data.get(coord);
                wp.elevation = elev.map(|x| *x as f64);
            }
        });
}
