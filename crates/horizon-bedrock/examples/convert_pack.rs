//! Converts a Bedrock pack to a Java resource pack folder.
//!
//! `cargo run --release -p horizon-bedrock --example convert_pack -- <pack> <out> [subpack]`

use horizon_bedrock::convert::to_java;
use horizon_bedrock::pack::{Options, Pack};
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(pack), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: convert_pack <pack> <out> [subpack]");
        std::process::exit(2);
    };
    let started = Instant::now();
    let pack = Pack::open_with(
        Path::new(pack),
        &Options {
            subpack: args.get(2).cloned(),
        },
    )
    .expect("open");
    let report = to_java(&pack, Path::new(out), (64, 0)).expect("convert");
    println!("{report:?} in {:?}", started.elapsed());
}
