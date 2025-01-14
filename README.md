# [fit2gpx-rs](https://github.com/jeromeschmied/fit2gpx-rs): convert .fit to .gpx files efficiently

## Installation

1.  have a Rust supported platform, eg.: linux, macos, windows
2.  have [Rust](https://rust-lang.org) installed
3.  install
    -   with `cargo` from [crates.io](https://crates.io): `cargo install fit2gpx`
    -   with `cargo` from [source](https://github.com/jeromeschmied/fit2gpx-rs): `cargo install --locked --git "https://github.com/jeromeschmied/fit2gpx-rs"`
    -   with `git` and `cargo`: cloning, then building from [source](https://github.com/jeromeschmied/fit2gpx-rs):

> ```sh
> git clone --depth 1 "https://github.com/jeromeschmied/fit2gpx-rs"
> cd fit2gpx-rs
> cargo install --locked --path .
> # without installing to ...cargo/bin/fit2gpx: cargo r (--release) -- -h
> ```

> [!NOTE]
> soon binary releases will be provided for linux, macos and windows

## Usage

### binary

see `fit2gpx --help`

let's say you want to convert `a_lovely_evening_walk.fit` to `a_lovely_evening_walk.gpx`
in that case, you'd do the following
`fit2gpx a_lovely_evening_walk.fit`
if you also want to add elevation data, as the `.fit` file didn't contain any, follow [these steps](#how-to-add-elevation-data)

### library

short:

```rust
fit2gpx::convert_file("walk.fit").unwrap();
```

see [docs](https://docs.rs/crate/fit2gpx) or [examples](https://github.com/jeromeschmied/fit2gpx-rs/tree/main/examples) for more detailed usage

## Purpose

This is a simple Rust library and binary for converting .FIT files to .GPX files.
I've mostly written it, being fed up waiting for conversion of a strava bulk export while creating
awesome plots with this great tool: [stravavis](https://github.com/marcusvolz/strava_py).

A **_significantly_** faster alternative to the great [**_fit2gpx_**](https://github.com/dodo-saba/fit2gpx)
with the ability to add elevation data while converting

-   [FIT](https://developer.garmin.com/fit/overview/) is a GIS data file format used by Garmin GPS sport devices and Garmin software
-   [GPX](https://docs.fileformat.com/gis/gpx/) is an XML based format for GNSS tracks

## Is it any good?

Yes.

## Why

-   it's damn fast
-   it can add elevation data
-   should be fairly well maintained
-   providing Rust library
-   it's fun

## How to add elevation data

-   first of all, have srtm data: `.hgt` files downloaded
    one great source is [Sonny's collection](https://sonny.4lima.de/), it's only for Europe though
-   then unzip everything, place all of the `.hgt` files to a single directory
-   set `$ELEV_DATA_DIR` to that very directory or pass `--elev_data_dir ~/my_elevation_data_dir`
-   make sure that `elevation` feature is enabled, _it's the default_
-   pass the `--add_elevation | -a` flag to `fit2gpx`

## Why might this one not be the right choice

### it doesn't support strava bulk-export stuff

-   unzipping `.gz` files. __solution__: in your activities directory run `gzip -d *.gz`
-   adding metadata to gpx files from the `activities.csv` file

## Direct dependencies

<!-- -   [coordinate-altitude](https://github.com/jeromeschmied/coordinate-altitude) -->

-   [fit_file](https://crates.io/crates/fit_file): reading .fit
-   [gpx](https://crates.io/crates/gpx): writing .gpx
-   [clap](https://crates.io/crates/clap): argument parsing
-   [rayon](https://crates.io/crates/rayon): multi-threadedness
-   [srtm](https://github.com/jeromeschmied/srtm_reader): reading elevation data from SRTM files
