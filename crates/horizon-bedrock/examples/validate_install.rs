//! Installs one resource pack folder in every container the game reads and checks that each
//! converts to the same Java pack.
//!
//! `cargo run --release -p horizon-bedrock --example validate_install -- <pack folder>`
//!
//! Given a file (`.mcpack`, `.mcaddon`, ...), converts it and prints what it holds.

use horizon_bedrock::bundle::Bundle;
use horizon_bedrock::convert::to_java;
use horizon_bedrock::pack::{Options, Pack, order_by_dependencies};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, p));
        }
    }
}

fn zip_of(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        for (name, bytes) in files {
            z.start_file(name.as_str(), o).unwrap();
            z.write_all(bytes).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn listing(dir: &Path) -> BTreeMap<String, u64> {
    let mut v = Vec::new();
    walk(dir, dir, &mut v);
    v.into_iter()
        .map(|(rel, p)| (rel, fs::metadata(p).unwrap().len()))
        .collect()
}

fn main() {
    let src = PathBuf::from(std::env::args().nth(1).expect("usage: validate_install <pack folder>"));
    if src.is_file() {
        let packs = Pack::open_all(&src, &Options::default()).expect("open");
        let (packs, missing) = order_by_dependencies(packs);
        for p in &packs {
            let m = p.manifest();
            println!("pack {} {:?} resource={} files={}", m.name, m.version, m.is_resource_pack(), p.paths().count());
        }
        for (p, d) in &missing {
            println!("missing dependency {d} of {p}");
        }
        let out = std::env::temp_dir().join(format!("horizon-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        for p in packs.iter().filter(|p| p.manifest().is_resource_pack()) {
            let r = to_java(p, &out, (64, 0)).expect("convert");
            println!("{r:?}");
        }
        let b = Bundle::load(&out);
        println!("entities {} attachables {} geometries {} settings {} files {}", b.entities.len(), b.attachables.len(), b.geometries.len(), b.settings.len(), listing(&out).len());
        let _ = fs::remove_dir_all(&out);
        return;
    }
    let work = std::env::temp_dir().join(format!("horizon-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).unwrap();
    let mut files = Vec::new();
    walk(&src, &src, &mut files);
    let rp: Vec<(String, Vec<u8>)> = files
        .iter()
        .map(|(r, p)| (r.clone(), fs::read(p).unwrap()))
        .collect();
    let bp_manifest = br#"{"format_version":2,"header":{"name":"bp","description":"","uuid":"00000000-0000-4000-8000-0000000000b0","version":[1,0,0],"min_engine_version":[1,20,0]},"modules":[{"type":"data","uuid":"00000000-0000-4000-8000-0000000000b1","version":[1,0,0]}]}"#.to_vec();
    let with_prefix = |prefix: &str, f: &[(String, Vec<u8>)]| -> Vec<(String, Vec<u8>)> {
        f.iter().map(|(n, b)| (format!("{prefix}{n}"), b.clone())).collect()
    };
    let mcpack = zip_of(&rp);
    let mut variants: Vec<(&str, PathBuf)> = Vec::new();
    let mut put = |name: &'static str, file: &str, bytes: Vec<u8>| {
        let p = work.join(file);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, bytes).unwrap();
        variants.push((name, p));
    };
    put("mcpack (files at root)", "a.mcpack", mcpack.clone());
    put("zip (wrapper folder)", "b.zip", zip_of(&with_prefix("Wrapper/", &rp)));
    let mut addon = with_prefix("RP/", &rp);
    addon.push(("BP/manifest.json".into(), bp_manifest.clone()));
    put("mcaddon (RP + BP folders)", "c.mcaddon", zip_of(&addon));
    put(
        "mcaddon (nested mcpack)",
        "d.mcaddon",
        zip_of(&[("rp.mcpack".into(), mcpack.clone()), ("bp.mcpack".into(), zip_of(&[("manifest.json".into(), bp_manifest.clone())]))]),
    );
    let mut world = with_prefix("resource_packs/Demo/", &rp);
    world.push(("level.dat".into(), vec![0; 8]));
    world.push(("behavior_packs/bp/manifest.json".into(), bp_manifest.clone()));
    put("mcworld", "e.mcworld", zip_of(&world));
    put("mctemplate", "f.mctemplate", zip_of(&world));
    let backslash: Vec<(String, Vec<u8>)> = rp.iter().map(|(n, b)| (format!("Demo\\{}", n.replace('/', "\\")), b.clone())).collect();
    put("zip (backslash names)", "g.mcpack", zip_of(&backslash));

    let convert = |path: &Path, out: &Path| -> Result<(usize, usize), String> {
        let packs = Pack::open_all(path, &Options::default()).map_err(|e| e.to_string())?;
        let (packs, _) = order_by_dependencies(
            packs.into_iter().filter(|p| p.manifest().is_resource_pack()).collect(),
        );
        if packs.is_empty() {
            return Err("no resource pack".into());
        }
        for p in &packs {
            to_java(p, out, (64, 0)).map_err(|e| e.to_string())?;
        }
        let b = Bundle::load(out);
        Ok((b.entities.len(), b.attachables.len()))
    };
    let base_out = work.join("out-folder");
    let base = convert(&src, &base_out).expect("folder");
    let base_list = listing(&base_out);
    println!("{:<30} entities {:>4} attachables {:>4} files {:>5}", "folder", base.0, base.1, base_list.len());
    let mut bad = 0;
    for (name, path) in &variants {
        let out = work.join(format!("out-{}", path.file_name().unwrap().to_string_lossy()));
        match convert(path, &out) {
            Ok(n) => {
                let l = listing(&out);
                let same = l == base_list && n == base;
                println!("{:<30} entities {:>4} attachables {:>4} files {:>5}  {}", name, n.0, n.1, l.len(), if same { "OK" } else { "DIFF" });
                if !same {
                    bad += 1;
                }
            }
            Err(e) => {
                println!("{name:<30} ERROR {e}");
                bad += 1;
            }
        }
    }
    let _ = fs::remove_dir_all(&work);
    if bad > 0 {
        std::process::exit(1);
    }
}
