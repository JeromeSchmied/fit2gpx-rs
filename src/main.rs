use clap::Parser;
#[cfg(feature = "elevation")]
use fit2gpx::elevation::*;
use fit2gpx::{fit::Fit, Res};
use rayon::prelude::*;

mod args;

fn main() -> Res<()> {
    // collecting cli args
    let conf = args::Cli::parse();
    // TODO: appropriate logging
    #[cfg(feature = "elevation")]
    {
        dbg!(&conf.elev_data_dir);
        dbg!(&conf.add_elevation);
    }
    dbg!(&conf.overwrite);

    // reading all .fit files into memory, considering whether it should be overwritten
    let all_fit = conf
        .files
        .par_iter()
        .filter(|f| {
            f.extension().is_some_and(|x| x == "fit")
                && (conf.overwrite || !f.with_extension("gpx").exists())
        })
        .flat_map(|f| Fit::from_file(f).inspect_err(|e| eprintln!("read error: {e:?}")))
        .collect::<Vec<_>>();

    #[cfg(feature = "elevation")]
    // collecting all needed tiles' coordinates, if adding elevation
    let all_needed_tile_coords = if conf.add_elevation {
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
                dbg!(&fit.file_name);
                add_elev_unchecked(
                    &mut fit.track_segment.points,
                    &all_elev_data,
                    conf.overwrite,
                );
            }
            if !fit.track_segment.points.is_empty() {
                fit.save_to_gpx()
                    .inspect_err(|e| eprintln!("conversion error: {e:?}"))
                    .map_err(|_| "conversion error")
            } else {
                eprintln!("{:?}: empty trkseg, ignoring...", fit.file_name);
                Ok(())
            }
        })?;

    Ok(())
}
