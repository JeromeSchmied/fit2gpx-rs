use clap::Parser;
#[cfg(feature = "elevation")]
use fit2gpx::elevation::*;
use fit2gpx::{fit::Fit, Res};
use rayon::prelude::*;

mod args;

fn main() -> Res<()> {
    // env_logger::init();
    env_logger::Builder::default()
        .filter(None, log::LevelFilter::Info)
        .init();

    // collecting cli args
    let conf = args::Cli::parse();
    // TODO: appropriate logging
    #[cfg(feature = "elevation")]
    {
        log::info!("should add elevation: {:?}", conf.add_elevation);
        log::info!("elevation data directory: {:?}", conf.elev_data_dir);
    }
    log::info!("should overwrite existing gpx: {}", conf.overwrite);

    // reading all .fit files into memory, considering whether it should be overwritten
    let all_fit = conf
        .files
        .par_iter()
        .filter(|f| {
            let is_fit = f.extension().is_some_and(|x| x == "fit");
            let converted = f.with_extension("gpx").exists();
            let remains = is_fit && (conf.overwrite || !converted);
            if !remains {
                log::warn!(
                    "ignoring {f:?}: extension is fit: {is_fit}, converted already: {converted}"
                );
            } else {
                log::debug!("loading {f:?} into memory");
            }
            remains
        })
        .flat_map(|f| Fit::from_file(f).inspect_err(|e| log::error!("read error: {e:?}")))
        .collect::<Vec<_>>();

    #[cfg(feature = "elevation")]
    // collecting all needed tiles' coordinates, if adding elevation
    let all_needed_tile_coords = if conf.add_elevation {
        log::info!("loading needed tiles' coordinates");
        let mut all = all_fit
            .par_iter()
            .flat_map(|fit| needed_tile_coords(&fit.track_segment.points))
            .collect::<Vec<_>>();
        all.sort_unstable();
        all.dedup();

        all
    } else {
        vec![]
    };
    #[cfg(feature = "elevation")]
    // reading all needed tiles to memory
    let all_needed_tiles = read_needed_tiles(&all_needed_tile_coords, conf.elev_data_dir);
    #[cfg(feature = "elevation")]
    // merging coordinates and tiles into a `HashMap`
    let all_elev_data = get_all_elev_data(&all_needed_tile_coords, &all_needed_tiles);

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
            log::info!("converting {:?}", fit.file_name);
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
