# fit2gpx-rs

## Purpose

This is a simple Rust library and binary for converting .FIT files to .GPX files.
A **_faster_** alternative to [fit2gpx](https://github.com/dodo-saba/fit2gpx)

-   [FIT](https://developer.garmin.com/fit/overview/) is a GIS data file format used by Garmin GPS sport devices and Garmin software
-   [GPX](https://docs.fileformat.com/gis/gpx/) is an XML based format for GPS tracks.

## Why another one

-   it's about 80 times as fast (single file, no elevation added)
-   it can add elevation data (though it isn't very precise)
-   it's fun

## Why not this one

-   it doesn't support strava bulk-export stuff

## Direct dependencies

-   [coordinate-altitude](https://github.com/jeromeschmied/coordinate-altitude)
-   [fit_file](https://crates.io/crates/fit_file)
-   [gpx](https://crates.io/crates/gpx)
-   [clap](https://crates.io/crates/clap)
