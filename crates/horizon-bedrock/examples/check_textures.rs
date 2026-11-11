//! Which Bedrock block and item textures have no sprite in the game: reads one texture path per
//! line (`textures/items/record_13`) and the game's sprite names (`item/music_disc_13`), prints
//! the paths `convert::java_texture` maps to a sprite that does not exist.
//!
//!   cargo run --release -p horizon-bedrock --example check_textures -- <sprites.txt> <paths.txt>

use horizon_bedrock::convert::java_texture;
use std::collections::BTreeSet;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(sprites), Some(paths)) = (args.next(), args.next()) else {
        eprintln!("usage: check_textures <sprites.txt> <paths.txt>");
        return;
    };
    let sprites: BTreeSet<String> = std::fs::read_to_string(sprites)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let (mut found, mut lost) = (0, Vec::new());
    for path in std::fs::read_to_string(paths).unwrap_or_default().lines() {
        match java_texture(path) {
            Some(java) if sprites.contains(&java) => found += 1,
            Some(java) => lost.push(format!("{path} -> {java}")),
            None => lost.push(format!("{path} -> none")),
        }
    }
    println!("{found} found, {} without a sprite", lost.len());
    for l in lost {
        println!("{l}");
    }
}
