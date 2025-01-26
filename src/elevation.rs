use crate::{utils, Res};
use gpx::Waypoint;
use rayon::prelude::*;
pub use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

/// truncated coordinate
pub type Coord = (i8, i16);
/// loaded elevation data, consisting of [`Coord`]s mapped to [`srtm_reader::Tile`]
pub type ElevData = HashMap<Coord, srtm_reader::Tile>;

/// collect all the [`srtm_reader::Tile`]'s coordinates that shall be loaded into memory
/// in order to be able to get elevation data for all `wps`
pub fn needed_tile_coords(wps: &[Waypoint]) -> BTreeSet<Coord> {
    // kinda Waypoint to Coord
    let trunc = |wp: &Waypoint| -> Coord {
        let (x, y) = wp.point().x_y();
        (y.trunc() as i8, x.trunc() as i16)
    };
    // tiles we need
    wps.par_iter()
        .filter(|wp| !utils::is_00(wp))
        .map(trunc)
        .collect()
}

/// read HGT [`srtm_reader::Tile`]s specified by `needs`, from `elev_data_dir`
/// **_NOTE_**: if a [`srtm_reader::Tile`] can't be loaded, it won't be
pub fn read_needed_tiles(
    needs: &BTreeSet<Coord>,
    elev_data_dir: impl AsRef<Path>,
) -> Vec<srtm_reader::Tile> {
    log::info!("reading needed tiles into memory");
    log::debug!("needed tiles' coordinates are: {needs:?}");

    let elev_data_dir = elev_data_dir.as_ref();
    needs
        .par_iter()
        .map(|c| srtm_reader::Coord::from(*c).get_filename())
        .map(|t| elev_data_dir.join(t))
        .flat_map(|p| {
            srtm_reader::Tile::from_file(&p)
                .inspect_err(|e| log::error!("error while reading {p:?} into memory: {e:#?}"))
        })
        .collect()
}
/// index the [`srtm_reader::Tile`]s with their coordinates
pub fn index_tiles(tiles: Vec<srtm_reader::Tile>) -> ElevData {
    log::info!("indexing all dem tiles");
    log::trace!("tiles: {tiles:?}");
    tiles
        .into_par_iter()
        .map(|tile| ((tile.latitude, tile.longitude), tile))
        .collect()
    // log::debug!("loaded elevation data: {:?}", all_elev_data.keys());
}
impl crate::Fit {
    /// add elevation data to the `fit` struct, reading hgt DTM from `elev_data_dir`
    /// # usage
    /// write manually, utilizing [`Self::add_elev_loaded`], instead of using in batch,
    /// as loading DTM is relatively slow, reading it once and storing it can greatly increase performance
    /// **_NOTE_**: if DTM can't be loaded, it will NOT be added
    pub fn add_elev_read(&mut self, elev_data_dir: impl AsRef<Path>, overwrite: bool) -> Res<()> {
        use super::elevation::*;
        let needed_tile_coords = needed_tile_coords(&self.track_segment.points);
        let needed_tiles = read_needed_tiles(&needed_tile_coords, elev_data_dir);
        let all_elev_data = index_tiles(needed_tiles);

        self.add_elev_loaded(&all_elev_data, overwrite)
    }
    /// add elevation to `self` using already loaded `elev_data` in parallel
    /// **_NOTE_**: if a needed [`srtm_reader::Tile`] is NOT loaded, elevation will NOT be added
    ///
    /// # Usage
    ///
    /// using the following order, it should be safe
    ///
    /// ```no_run
    /// use fit2gpx::elevation;
    ///
    /// let mut fit = fit2gpx::Fit::from_file("evening_walk.fit").unwrap();
    /// let elev_data_dir = "~/Downloads/srtm_data";
    /// let needed_tile_coords = elevation::needed_tile_coords(&fit.track_segment.points);
    /// let needed_tiles = elevation::read_needed_tiles(&needed_tile_coords, elev_data_dir);
    /// let all_elev_data = elevation::index_tiles(needed_tiles);
    ///
    /// fit.add_elev_loaded(&all_elev_data, false);
    /// ```
    pub fn add_elev_loaded(&mut self, elev_data: &ElevData, overwrite: bool) -> Res<()> {
        // coord is (x;y) but we need (y;x)
        let xy_yx = |wp: &Waypoint| -> srtm_reader::Coord {
            let (x, y) = wp.point().x_y();
            (y, x).into()
        };
        Ok(self
            .track_segment
            .points
            .par_iter_mut()
            .filter(|wp| (wp.elevation.is_none() || overwrite) && !utils::is_00(wp))
            .try_for_each(|wp| {
                let coord = xy_yx(wp);
                if let Some(elev_data) = elev_data.get(&coord.trunc()) {
                    let elev = elev_data.get(coord);
                    wp.elevation = elev.map(|x| *x as f64);
                    Ok(())
                } else {
                    Err(format!("`elev_data` didn't contain {coord:?}"))
                }
            })?)
    }
}
