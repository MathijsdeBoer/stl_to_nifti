use std::env;
use std::fmt::Write;
use std::fs::read;
use std::path::Path;
use std::time::{Duration, Instant};

extern crate ndarray;
extern crate indicatif;
extern crate nifti;
extern crate pk_stl;

use indicatif::{
    ParallelProgressIterator,
    ProgressBar,
    ProgressIterator,
    ProgressState,
    ProgressStyle
};
use ndarray::Array3;
use ndarray::parallel::prelude::*;
use nifti::{NiftiObject, ReaderOptions};
use nifti::writer::WriterOptions;
use pk_stl::{parse_stl, StlModel};
use pk_stl::geometry::Vec3;



fn _within_bounds(point: Vec3, bounds: ((f32, f32), (f32, f32), (f32, f32))) -> bool {
    point.x > bounds.0.0 && point.x < bounds.0.1 &&
        point.y > bounds.1.0 && point.y < bounds.1.1 &&
        point.z > bounds.2.0 && point.z < bounds.2.1
}


fn contains(mesh: &StlModel, point: Vec3) -> u8 {
// Bounds check
    u8::from(_within_bounds(point, mesh.dimension_range().unwrap()))
// TODO: if inside bounds, check if inside mesh
}


fn format_duration(duration: Duration) -> String {
    let milli = duration.as_millis() % 1000;
    let s = duration.as_secs() % 60;
    let m = (duration.as_secs() / 60) % 60;
    let h = (duration.as_secs() / 60) / 60;
    format!("{h:0>2}h:{m:0>2}m:{s:0>2}.{milli:0>4}s")
}


fn main() {
    println!("Hello, world!");

    let args: Vec<String> = env::args().collect();
    let stl_path = Path::new(&args[1]);
    let out_path = Path::new(&args[2]);
    let ref_path = Path::new(&args[3]);

    println!("STL: {}", args[1]);
    println!("OUT: {}", args[2]);
    println!("REF: {}", args[3]);

    println!("Reading STL");
    let mut start = Instant::now();
    let stl_bytes: &[u8] = &read(stl_path).unwrap();
    let stl = parse_stl(stl_bytes).unwrap();
    let mut duration = start.elapsed();
    println!("Done in {}", format_duration(duration));
    println!("STL bounds: {:?}", stl.dimension_range().unwrap());


    println!("Reading REF");
    start = Instant::now();
    let ref_nii = ReaderOptions::new().read_file(ref_path).unwrap();
    println!("Extracting header from REF");
    let header = ref_nii.header();
    let total_spacing = header.pixdim;
    let total_shape = header.dim;
    duration = start.elapsed();
    println!("Done in {}", format_duration(duration));

    println!("REF: shape {:?}", total_shape);
    println!("REF: Spacing {:?}", total_spacing);

    let n_dim = total_shape[0] as usize;
    println!("REF: dimensions {:?}", n_dim);
    let shape = &total_shape[1..(n_dim + 1)];
    println!("REF: shape after dimension selection {:?}", shape);
    let spacing = &total_spacing[1..(n_dim + 1)];
    println!("REF: spacing after dimension selection {:?}", spacing);

    println!("Generating coordinates...");
    let mut coordinates: Vec<[usize; 3]> = vec![];
    for z in (0..shape[2]).progress() {
        for y in 0..shape[1] {
            for x in 0..shape[0] {
                coordinates.push([x as usize, y as usize, z as usize]);
            }
        }
    }
    println!("Done generating {} coordinates", coordinates.len());

    let pb = ProgressBar::new(coordinates.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {human_pos:>12} / {human_len:>12} ({percent}%)({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{}", format_duration(state.eta())).unwrap())
            .progress_chars("#>-")
    );

    let results: Vec<u8> = coordinates.par_iter().progress_with(pb).map(|coord| {
        let point = Vec3 {
            x: coord[0] as f32 * spacing[0],
            y: coord[1] as f32 * spacing[1],
            z: coord[2] as f32 * spacing[2],
        };

        contains(&stl, point)
    }).collect();

    println!("{:?} results", results.len());

    println!("ARR: Creating");
    let mut arr = Array3::<u8>::zeros((shape[2] as usize, shape[1] as usize, shape[0] as usize));
    let n_elem = arr.len();
    println!("ARR: {:?} elements", n_elem);
    println!("ARR: {:?} dims", arr.ndim());
    println!("ARR: shape {:?}", arr.shape());

    for (i, coords) in coordinates.iter().enumerate().progress() {
        arr[[coords[0], coords[1], coords[2]]] = results[i];
    }

    WriterOptions::new(out_path).write_nifti(&arr).expect("TODO: panic message");
}
