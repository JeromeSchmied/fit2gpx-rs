use clap::Parser;
#[cfg(feature = "elevation")]
use fit2gpx::elevation::*;
use fit2gpx::{fit::Fit, Res};
use rayon::prelude::*;

mod args;

fn main() -> Res<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // collecting cli args
    let conf = args::Cli::parse();
    log::debug!("cli args: {conf:?}");
    log::info!("should overwrite existing gpx: {}", conf.overwrite);

    // reading all .fit files into memory, considering whether it should be overwritten
    let all_fit = conf
        .files
        .par_iter()
        .filter(|f| {
            let is_fit = f.extension().is_some_and(|x| x == "fit");
            let converted = f.with_extension("gpx").exists();
            let remains = is_fit && (conf.overwrite || !converted);
            if remains {
                log::debug!("loading {f:?} into memory");
            } else {
                log::warn!(
                    "ignoring {f:?}: extension is fit: {is_fit}, converted already: {converted}"
                );
            }
            remains
        })
        .flat_map(|f| Fit::from_file(f).inspect_err(|e| log::error!("read error: {e:?}")))
        .collect::<Vec<_>>();

    #[cfg(feature = "elevation")]
    let all_elev_data = read_elev_data(&conf, &all_fit);

    // iterating over all .fit files that are in memory in parallel
    // adding elevation data if requested
    // converting to .gpx and writing it to disk
    all_fit
        .into_par_iter()
        .try_for_each(|mut fit: Fit| -> Result<(), &'static str> {
            #[cfg(feature = "elevation")]
            if conf.add_elevation {
                log::debug!("adding elevation to {:?}", fit.file_name);
                add_elev_unchecked(
                    &mut fit.track_segment.points,
                    &all_elev_data,
                    conf.overwrite,
                );
            }
            log::debug!("converting {:?}", fit.file_name);
            if fit.track_segment.points.is_empty() {
                log::warn!("{:?}: empty trkseg, ignoring...", fit.file_name);
                Ok(())
            } else {
                fit.save_to_gpx()
                    .inspect_err(|e| log::error!("conversion error: {e:?}"))
                    .map_err(|_| "conversion error")
            }
        })?;

    Ok(())
}

#[cfg(feature = "elevation")]
fn read_elev_data(conf: &args::Cli, all_fit: &Vec<Fit>) -> HashMap<(i32, i32), srtm_reader::Tile> {
    log::info!("should add elevation: {:?}", conf.add_elevation);
    if !conf.add_elevation {
        return HashMap::new();
    }
    log::info!("elevation data directory: {:?}", conf.elev_data_dir);

    // collecting all needed tiles' coordinates, if adding elevation
    let all_needed_tile_coords = if conf.add_elevation {
        log::info!("loading needed tiles' coordinates");
        let all = all_fit
            .par_iter()
            .flat_map(|fit| needed_tile_coords(&fit.track_segment.points))
            .collect::<BTreeSet<_>>();
        log::debug!("loaded these tiles: {all:?}");

        all
    } else {
        BTreeSet::new()
    };
    // reading all needed tiles to memory
    let all_needed_tiles = read_needed_tiles(&all_needed_tile_coords, &conf.elev_data_dir);
    // merging coordinates and tiles into a `HashMap`
    index_tiles(all_needed_tiles)
}
