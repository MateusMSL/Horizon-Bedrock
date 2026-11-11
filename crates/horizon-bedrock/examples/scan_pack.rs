//! Opens a Bedrock pack and checks that this crate reads all of it: lists what the pack holds,
//! parses every JSON file, and parses every Molang expression in them.
//!
//! `cargo run --release -p horizon-bedrock --example scan_pack -- <pack> [subpack]`
//!
//! Third-party packs are never part of the repository: pass your own.

use horizon_bedrock::molang;
use horizon_bedrock::pack::{Options, Pack};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

/// A string that holds Molang: a namespace word (`q.`, `query.`, `v.`, `variable.`, `t.`,
/// `temp.`, `c.`, `context.`, `math.`) not preceded by a name character.
fn looks_like_molang(s: &str) -> bool {
    let b = s.as_bytes();
    for namespace in [
        "query.",
        "variable.",
        "temp.",
        "context.",
        "math.",
        "q.",
        "v.",
        "t.",
        "c.",
    ] {
        let mut from = 0;
        while let Some(at) = s[from..].find(namespace) {
            let at = from + at;
            let before = if at == 0 { b' ' } else { b[at - 1] };
            let after = b.get(at + namespace.len()).copied().unwrap_or(b' ');
            if !(before.is_ascii_alphanumeric()
                || matches!(before, b'_' | b'/' | b'.' | b':' | b'-'))
                && (after.is_ascii_alphabetic() || after == b'_')
            {
                return true;
            }
            from = at + namespace.len();
        }
    }
    false
}

fn walk(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) if looks_like_molang(s) => out.push(s.clone()),
        Value::Array(a) => {
            // A block a script opens on one line and closes on a later one is one expression.
            let (mut group, mut depth) = (String::new(), 0);
            for x in a {
                match x {
                    Value::String(s) if looks_like_molang(s) || depth > 0 => {
                        if !group.is_empty() {
                            group.push('\n');
                        }
                        group.push_str(s);
                        depth += horizon_bedrock::animation::open_depth(s);
                        if depth <= 0 {
                            out.push(std::mem::take(&mut group));
                            depth = 0;
                        }
                    }
                    other => walk(other, out),
                }
            }
            if !group.is_empty() {
                out.push(group);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                if looks_like_molang(k) {
                    out.push(k.clone());
                }
                walk(x, out);
            }
        }
        _ => {}
    }
}

/// Prints the crate's warnings (a Molang line that does not parse, an animation that plays itself).
struct Warnings;
impl log::Log for Warnings {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record) {
        eprintln!("warning: {}", r.args());
    }
    fn flush(&self) {}
}

fn main() {
    let _ = log::set_logger(&Warnings).map(|()| log::set_max_level(log::LevelFilter::Warn));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: scan_pack <pack> [subpack]");
        std::process::exit(2);
    };
    let started = Instant::now();
    let pack = Pack::open_with(
        Path::new(path),
        &Options {
            subpack: args.get(1).cloned(),
        },
    )
    .expect("open");
    let m = pack.manifest();
    println!(
        "{} {} (format {}, min engine {:?})",
        m.name,
        m.version,
        m.format_version,
        m.min_engine_version.map(|v| v.to_string())
    );
    println!(
        "modules: {:?}",
        m.modules
            .iter()
            .map(|m| m.type_name.as_str())
            .collect::<Vec<_>>()
    );
    println!("unsupported modules: {}", pack.unsupported_modules().len());
    println!(
        "subpacks: {:?}",
        m.subpacks
            .iter()
            .map(|s| s.folder_name.as_str())
            .collect::<Vec<_>>()
    );
    println!("capabilities: {:?}", m.capabilities);

    let mut by_dir: BTreeMap<String, usize> = BTreeMap::new();
    for p in pack.paths() {
        *by_dir
            .entry(p.split('/').next().unwrap_or("").to_owned())
            .or_default() += 1;
    }
    println!("files by top folder: {by_dir:?}");
    println!("opened in {:?}", started.elapsed());

    let started = Instant::now();
    let (mut json_ok, mut json_bad) = (0, 0);
    let mut sources = Vec::new();
    for p in pack
        .paths()
        .filter(|p| p.ends_with(".json") || p.ends_with(".material"))
        .map(str::to_owned)
        .collect::<Vec<_>>()
    {
        match pack.read_json(&p) {
            Ok(v) => {
                json_ok += 1;
                walk(&v, &mut sources);
            }
            Err(e) => {
                json_bad += 1;
                println!("json: {e}");
            }
        }
    }
    println!(
        "json: {json_ok} ok, {json_bad} failed, in {:?}",
        started.elapsed()
    );

    let started = Instant::now();
    let total = sources.len();
    let bytes: usize = sources.iter().map(String::len).sum();
    let mut failures: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut nodes = 0usize;
    for s in &sources {
        match molang::parse(s) {
            Ok(p) => nodes += p.body.len(),
            Err(e) => {
                let entry = failures
                    .entry(e.message.clone())
                    .or_insert((0, String::new()));
                entry.0 += 1;
                if entry.1.is_empty() {
                    let from = e.at.saturating_sub(40);
                    let to = (e.at + 40).min(s.len());
                    entry.1 = format!(
                        "...{}<<HERE>>{}...",
                        s.get(from..e.at).unwrap_or(""),
                        s.get(e.at..to).unwrap_or("")
                    );
                }
            }
        }
    }
    let failed: usize = failures.values().map(|f| f.0).sum();
    // Every parsed expression once against a host that knows nothing: no panic, no hang.
    let started_eval = Instant::now();
    let programs: Vec<_> = sources
        .iter()
        .filter_map(|s| molang::parse(s).ok())
        .collect();
    let mut rng = molang::Rng::new(1);
    let mut vars = molang::Vars::default();
    let mut host = molang::NoHost;
    for p in &programs {
        p.eval(&mut molang::EvalCtx {
            host: &mut host,
            vars: &mut vars,
            rng: &mut rng,
            this: molang::Value::Num(0.0),
        });
    }
    println!(
        "evaluated {} expressions once in {:?}",
        programs.len(),
        started_eval.elapsed()
    );
    println!(
        "molang: {total} expressions, {bytes} bytes, {} parsed ({nodes} statements), {failed} failed, in {:?}",
        total - failed,
        started.elapsed()
    );
    for (message, (n, example)) in &failures {
        println!("  {n} x {message}: {example}");
    }

    // Models, animations and controllers.
    let started = Instant::now();
    let mut geos = 0usize;
    let mut quads = 0usize;
    let mut geo_bad = 0usize;
    let mut lib = horizon_bedrock::animation::Library::default();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("models/") || p.contains("/models/") {
            match horizon_bedrock::geometry::parse_file(&v) {
                Ok(list) => {
                    for g in &list {
                        geos += 1;
                        fn count(parts: &[horizon_bedrock::geometry::Part]) -> usize {
                            parts
                                .iter()
                                .map(|p| p.quads.len() + count(&p.children))
                                .sum()
                        }
                        quads += count(&g.to_tree());
                    }
                }
                Err(e) => {
                    geo_bad += 1;
                    println!("geometry {p}: {e}");
                }
            }
        }
        if p.starts_with("animations/") || p.starts_with("animation_controllers/") {
            lib.add_file(&v);
        }
    }
    println!(
        "geometry: {geos} models, {quads} quads, {geo_bad} files failed; animations: {}, controllers: {}, in {:?}",
        lib.animations.len(),
        lib.controllers.len(),
        started.elapsed()
    );

    entities(&pack, &lib);
    extras(&pack);
}

struct NameHost(String);

impl molang::Host for NameHost {
    fn query(&mut self, name: &str, args: &[molang::Value]) -> molang::Value {
        match name {
            "is_name_any" => molang::Value::bool(
                args.iter()
                    .any(|a| matches!(a, molang::Value::Str(s) if s.eq_ignore_ascii_case(&self.0))),
            ),
            _ => molang::Value::Null,
        }
    }
}

fn entities(pack: &Pack, lib: &horizon_bedrock::animation::Library) {
    use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
    let started = Instant::now();
    let mut rcs = std::collections::HashMap::new();
    let mut geos = std::collections::HashSet::new();
    let mut defs = Vec::new();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("render_controllers/") {
            RenderController::add_file(&v, &mut rcs);
        }
        if p.starts_with("models/") {
            if let Ok(list) = horizon_bedrock::geometry::parse_file(&v) {
                geos.extend(list.into_iter().map(|g| g.id.to_ascii_lowercase()));
            }
        }
        if p.starts_with("entity/") || p.starts_with("attachables/") {
            if let Some(d) = ClientEntity::parse(&v) {
                defs.push(d);
            }
        }
    }
    let assets = Assets {
        animations: lib,
        render_controllers: &rcs,
        geometries: None,
    };
    let (mut with_layers, mut none, mut missing_geo, mut missing_tex, mut layers_total) =
        (0, 0, 0, 0, 0);
    let mut names = std::collections::BTreeSet::new();
    for d in &defs {
        for n in ["", "Snowman", "zanta", "baby"] {
            let mut inst = Instance::new(1);
            let mut host = NameHost(n.to_owned());
            let mut frame = None;
            for _ in 0..3 {
                frame = Some(inst.update(d, &assets, &mut host, 0.05));
            }
            let f = frame.unwrap();
            if n.is_empty() {
                if f.layers.is_empty() {
                    none += 1
                } else {
                    with_layers += 1
                }
            }
            for l in &f.layers {
                layers_total += 1;
                if !geos.contains(&l.geometry) {
                    missing_geo += 1;
                    names.insert(format!("geo {}", l.geometry));
                }
                let t = &l.texture;
                if !["png", "tga"]
                    .iter()
                    .any(|e| pack.contains(&format!("{t}.{e}")))
                {
                    missing_tex += 1;
                    names.insert(format!("tex {t}"));
                }
            }
        }
    }
    println!(
        "entities+attachables: {} defs, {with_layers} draw with no name, {none} draw nothing; {layers_total} layers, {missing_geo} missing geometry, {missing_tex} missing texture, in {:?}",
        defs.len(),
        started.elapsed()
    );
    for n in names.iter().take(10) {
        println!("  {n}");
    }
}

fn extras(pack: &Pack) {
    use horizon_bedrock::material::Materials;
    use horizon_bedrock::{particle, sounds};
    let started = Instant::now();
    let mut defs = sounds::Definitions::default();
    let mut events = sounds::EventSounds::default();
    let mut mats = Materials::default();
    let (mut fx_total, mut fx_ok, mut sprites) = (0, 0, 0usize);
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if p.ends_with(".material") {
            if let Ok(v) = pack.read_json(&p) {
                mats.add_file(&v);
            }
            continue;
        }
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p == "sounds/sound_definitions.json" {
            defs.add_file(&v);
        } else if p == "sounds.json" {
            events.add_file(&v);
        } else if p.starts_with("particles/") {
            fx_total += 1;
            if let Some(fx) = particle::Effect::parse(&v) {
                fx_ok += 1;
                let mut e = particle::Emitter::new(std::sync::Arc::new(fx), 1);
                let mut out = Vec::new();
                for _ in 0..40 {
                    e.update(0.05, &mut molang::NoHost, &particle::NoWorld, &mut out);
                }
                sprites += e.sprites(&mut molang::NoHost).len();
            }
        }
    }
    let missing_files = defs
        .map
        .values()
        .flat_map(|d| d.variants.iter())
        .filter(|v| {
            !["ogg", "wav", "fsb", "mp3"]
                .iter()
                .any(|e| pack.contains(&format!("{}.{e}", v.name)))
        })
        .count();
    println!(
        "sounds: {} definitions ({missing_files} variants without a file), events: {}; particles: {fx_ok}/{fx_total} parsed, {sprites} sprites after 2 s; material sample entity_alphatest: {:?}; in {:?}",
        defs.map.len(),
        if events.is_empty() { "none" } else { "present" },
        mats.flags("entity_alphatest"),
        started.elapsed()
    );
}
