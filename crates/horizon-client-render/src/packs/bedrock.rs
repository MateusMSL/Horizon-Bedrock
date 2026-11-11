//! Bedrock resource packs in `resourcepacks/`, converted to Java packs
//! ([`horizon_bedrock::convert`]) so the rest of the pack machinery loads them as it loads any
//! other. The conversion is kept on disk and made again only when the source changes.

use horizon_bedrock::convert::{cache_name, to_java};
use horizon_bedrock::pack::{Options, Pack, order_by_dependencies};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// A subpack a Bedrock pack offers (the gear of Bedrock's pack list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subpack {
    pub folder: String,
    /// The name with its grey explanation (`\u{a7}8...`) cut off.
    pub name: String,
}

/// The game folder holding `dir` (the `resourcepacks` folder).
fn game_dir(dir: &Path) -> &Path {
    dir.parent().unwrap_or(Path::new("."))
}

/// The subpack chosen for the pack file `name` in `dir`.
pub fn chosen_subpack(dir: &Path, name: &str) -> Option<String> {
    horizon_bedrock::settings::Config::read(game_dir(dir)).subpack(name)
}

/// Choose the subpack of the pack file `name` in `dir` (`None`: the base pack alone).
pub fn choose_subpack(dir: &Path, name: &str, subpack: Option<&str>) -> std::io::Result<()> {
    let mut config = horizon_bedrock::settings::Config::read(game_dir(dir));
    config.set_subpack(name, subpack);
    config.write(game_dir(dir))
}

/// The value chosen for the setting `setting` of the pack file `name` in `dir`.
pub fn chosen_setting(
    dir: &Path,
    name: &str,
    setting: &str,
) -> Option<horizon_bedrock::settings::Choice> {
    horizon_bedrock::settings::Config::read(game_dir(dir)).choice(name, setting)
}

/// Save the value of the setting `setting` of the pack file `name` in `dir`.
pub fn choose_setting(
    dir: &Path,
    name: &str,
    setting: &str,
    choice: &horizon_bedrock::settings::Choice,
) -> std::io::Result<()> {
    let mut config = horizon_bedrock::settings::Config::read(game_dir(dir));
    config.set_choice(name, setting, choice);
    config.write(game_dir(dir))
}

/// The settings the Bedrock pack at `path` offers, labels in English.
pub fn settings(path: &Path) -> Vec<horizon_bedrock::settings::Def> {
    Pack::open_all(path, &Options::default())
        .ok()
        .and_then(|packs| {
            packs
                .iter()
                .find(|p| p.manifest().is_resource_pack())
                .map(horizon_bedrock::settings::of_pack)
        })
        .unwrap_or_default()
}

/// The subpacks the Bedrock pack at `path` offers, in manifest order.
pub fn subpacks(path: &Path) -> Vec<Subpack> {
    let Ok(packs) = Pack::open_all(path, &Options::default()) else {
        return Vec::new();
    };
    packs
        .iter()
        .find(|p| p.manifest().is_resource_pack())
        .map(|p| {
            p.manifest()
                .subpacks
                .iter()
                .map(|s| Subpack {
                    folder: s.folder_name.clone(),
                    name: if s.name.is_empty() {
                        s.folder_name.clone()
                    } else {
                        s.name
                            .split('\u{a7}')
                            .next()
                            .unwrap_or(&s.name)
                            .trim()
                            .to_owned()
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The choice folded into the cache key, so each subpack converts to its own folder.
fn subpack_key(subpack: Option<&str>) -> u64 {
    subpack.map_or(0, |s| {
        s.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        })
    })
}

/// The marker a finished conversion leaves, so an interrupted one is made again.
const DONE: &str = ".horizon-bedrock-done";

/// Whether the file name is a Bedrock pack's by extension alone: a pack, an add-on, or a world
/// or template that carries resource packs.
pub fn is_bedrock_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".mcpack", ".mcaddon", ".mcworld", ".mctemplate"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Where `path` (a folder, `.zip`, `.mcpack` or `.mcaddon`) would be converted to and a stamp
/// of its state, or `None` when it cannot be read.
fn target(path: &Path, cache: &Path, subpack: Option<&str>) -> Option<PathBuf> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    // A folder's own size says nothing: the manifest's modification time stands for it.
    let (modified, size) = if meta.is_dir() {
        let manifest = fs::metadata(path.join("manifest.json")).ok()?;
        (
            manifest
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(modified, |d| d.as_secs()),
            manifest.len(),
        )
    } else {
        (modified, meta.len())
    };
    Some(cache.join(cache_name(path, modified, size ^ subpack_key(subpack))))
}

/// The Java pack folder for the Bedrock pack at `path`, converting it into `cache` when it is
/// not there already. `None` when `path` is not a Bedrock resource pack.
pub fn convert(
    path: &Path,
    cache: &Path,
    format: (i64, i64),
    subpack: Option<&str>,
) -> Option<PathBuf> {
    if path.is_dir() && !path.join("manifest.json").is_file() {
        return None;
    }
    let out = target(path, cache, subpack)?;
    if out.join(DONE).is_file() {
        return Some(out);
    }
    let options = Options {
        subpack: subpack.map(str::to_owned),
    };
    // A subpack the pack does not have (an old choice) falls back to the base.
    let packs = match Pack::open_all(path, &options)
        .or_else(|_| Pack::open_all(path, &Options::default()))
    {
        Ok(packs) => packs,
        Err(e) => {
            log::warn!(
                "Horizon: the Bedrock pack {} cannot be read: {e}",
                path.display()
            );
            return None;
        }
    };
    let (packs, missing) = order_by_dependencies(
        packs
            .into_iter()
            .filter(|p| p.manifest().is_resource_pack())
            .collect(),
    );
    if packs.is_empty() {
        log::warn!(
            "Horizon: {} holds no Bedrock resource pack (behaviour packs and scripts are not read)",
            path.display()
        );
        return None;
    }
    for (pack, dependency) in &missing {
        log::warn!(
            "Horizon: the pack {pack} in {} depends on {dependency}, which is not in the file",
            path.display()
        );
    }
    for pack in &packs {
        if pack.manifest().needs_newer_engine() {
            log::warn!(
                "Horizon: the pack {} asks for a newer game than the formats known ({}): what it uses that is new is left out",
                pack.manifest().name,
                horizon_bedrock::manifest::ENGINE_VERSION
            );
        }
    }
    let packs: Vec<&Pack> = packs.iter().collect();
    let _ = fs::remove_dir_all(&out);
    for pack in packs {
        match to_java(pack, &out, format) {
            Ok(report) => log::info!(
                "Horizon: converted the Bedrock pack {}: {report:?}",
                path.display()
            ),
            Err(e) => {
                log::warn!(
                    "Horizon: could not convert the Bedrock pack {}: {e}",
                    path.display()
                );
                let _ = fs::remove_dir_all(&out);
                return None;
            }
        }
    }
    // Older conversions of the same pack are of no use.
    if let (Ok(entries), Some(keep)) = (fs::read_dir(cache), out.file_name()) {
        let stem = out
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.rsplit_once('-'))
            .map(|(s, _)| s.to_owned());
        for e in entries.flatten() {
            let n = e.file_name();
            if n != keep
                && stem.as_deref().is_some_and(|s| {
                    n.to_string_lossy()
                        .rsplit_once('-')
                        .is_some_and(|(o, _)| o == s)
                })
            {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
    fs::write(out.join(DONE), b"").ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "horizon-bedrock-pack-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn pack(dir: &Path) {
        fs::create_dir_all(dir.join("entity")).unwrap();
        fs::write(
            dir.join("manifest.json"),
            br#"{"format_version":2,"header":{"name":"Demo","uuid":"00000000-0000-0000-0000-000000000001","version":[1,0,0]},
                "modules":[{"type":"resources","uuid":"00000000-0000-0000-0000-000000000002","version":[1,0,0]}]}"#,
        )
        .unwrap();
        fs::write(dir.join("entity/a.entity.json"), b"{}").unwrap();
    }

    #[test]
    fn names() {
        assert!(is_bedrock_file("A.McPack") && is_bedrock_file("b.mcaddon"));
        assert!(is_bedrock_file("w.mcworld") && is_bedrock_file("t.MCTEMPLATE"));
        assert!(!is_bedrock_file("c.zip"));
    }

    #[test]
    fn converts_once_and_a_java_pack_is_not_touched() {
        let root = tmp("c");
        let src = root.join("resourcepacks/Demo");
        pack(&src);
        let cache = root.join(".horizon-cache/bedrock");
        let out = convert(&src, &cache, (64, 0), None).unwrap();
        assert!(out.join("pack.mcmeta").is_file() && out.join(DONE).is_file());
        // Same input, same folder, not made again (a file added to it survives).
        fs::write(out.join("marker"), b"x").unwrap();
        assert_eq!(convert(&src, &cache, (64, 0), None).unwrap(), out);
        assert!(out.join("marker").exists());
        // A Java pack has no manifest.
        let java = root.join("resourcepacks/Java");
        fs::create_dir_all(&java).unwrap();
        fs::write(java.join("pack.mcmeta"), b"{}").unwrap();
        assert!(convert(&java, &cache, (64, 0), None).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_chosen_subpack_converts_to_its_own_folder() {
        let root = tmp("s");
        let src = root.join("resourcepacks/Demo");
        pack(&src);
        let manifest = fs::read_to_string(src.join("manifest.json")).unwrap().replace(
            r#""modules""#,
            r#""subpacks":[{"folder_name":"2_e","name":"Full","memory_performance_tier":1}],"modules""#,
        );
        fs::write(src.join("manifest.json"), manifest).unwrap();
        fs::create_dir_all(src.join("subpacks/2_e/entity")).unwrap();
        fs::write(src.join("subpacks/2_e/entity/b.entity.json"), b"{}").unwrap();
        let dir = root.join("resourcepacks");
        let found = subpacks(&src);
        assert_eq!(
            found,
            vec![Subpack {
                folder: "2_e".into(),
                name: "Full".into()
            }]
        );
        assert_eq!(chosen_subpack(&dir, "Demo"), None);
        choose_subpack(&dir, "Demo", Some("2_e")).unwrap();
        assert_eq!(chosen_subpack(&dir, "Demo").as_deref(), Some("2_e"));
        let cache = root.join(".horizon-cache/bedrock");
        let base = convert(&src, &cache, (64, 0), None).unwrap();
        let full = convert(&src, &cache, (64, 0), Some("2_e")).unwrap();
        assert_ne!(base, full);
        choose_subpack(&dir, "Demo", None).unwrap();
        assert_eq!(chosen_subpack(&dir, "Demo"), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_converted_pack_is_listed_and_compatible() {
        let root = tmp("l");
        pack(&root.join("addons/Demo"));
        pack(&root.join("resourcepacks/Demo"));
        // Only `addons/` takes Bedrock's packs.
        let none =
            super::super::folder(&root.join("resourcepacks"), super::super::RESOURCE_FORMAT);
        assert!(none.is_empty(), "{none:?}");
        let packs = super::super::addons(&root.join("addons"));
        assert_eq!(packs.len(), 1, "{packs:?}");
        assert_eq!(packs[0].id, "addon/Demo");
        assert_eq!(
            packs[0].compatibility,
            crate::pack_screen::Compat::Compatible
        );
        let _ = fs::remove_dir_all(&root);
    }
}
