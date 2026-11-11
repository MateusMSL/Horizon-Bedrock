//! `manifest.json`: what a pack is, which modules it holds, which subpacks it offers.

use crate::{Error, Result, jsonc};
use serde_json::Value;

/// A `major.minor.patch` version; the manifest writes it as `[1, 2, 3]` or `"1.2.3"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Version(pub [u32; 3]);

impl Version {
    pub fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::Array(a) => {
                let mut out = [0u32; 3];
                for (slot, x) in out.iter_mut().zip(a) {
                    *slot = u32::try_from(x.as_u64()?).ok()?;
                }
                Some(Version(out))
            }
            Value::String(s) => {
                let core = s.split(['-', '+']).next()?;
                let mut out = [0u32; 3];
                for (slot, part) in out.iter_mut().zip(core.split('.')) {
                    *slot = part.trim().parse().ok()?;
                }
                Some(Version(out))
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}

/// The newest game version whose pack formats this crate reads (`min_engine_version` and the
/// `format_version` of its files). A pack asking for more still loads, with what is known.
pub const ENGINE_VERSION: Version = Version([1, 26, 30]);

/// What a module is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    /// Textures, models, animations, sounds, language: what this crate reads.
    Resources,
    /// A behaviour pack: entity components, blocks, items, loot. Never read.
    Data,
    /// The Script API. Never read.
    Script,
    /// Anything else (`skin_pack`, `world_template`, `interface`, ...).
    Other,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub kind: ModuleKind,
    /// The manifest's own word for the kind.
    pub type_name: String,
    pub uuid: String,
    pub version: Version,
}

#[derive(Debug, Clone)]
pub struct Subpack {
    pub folder_name: String,
    pub name: String,
    pub memory_performance_tier: u32,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    /// A pack's UUID, or a script module's name.
    pub id: String,
    pub version: Option<Version>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub format_version: u32,
    /// The header's name, usually a translation key (`pack.name`).
    pub name: String,
    pub description: String,
    pub uuid: String,
    pub version: Version,
    pub min_engine_version: Option<Version>,
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
    /// `pbr`, `raytraced`, `script_eval`, ...
    pub capabilities: Vec<String>,
    pub subpacks: Vec<Subpack>,
    /// The options the pack offers its player (labels still translation keys).
    pub settings: Vec<crate::settings::Def>,
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_json(&jsonc::parse(text)?)
    }

    pub fn from_json(root: &Value) -> Result<Self> {
        let bad = |m: &str| Error::Manifest(m.to_owned());
        let header = root.get("header").ok_or_else(|| bad("no header"))?;
        let uuid = str_of(header, "uuid");
        if uuid.is_empty() {
            return Err(bad("header has no uuid"));
        }
        let modules = root
            .get("modules")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|m| {
                        let type_name = str_of(m, "type");
                        let kind = match type_name.as_str() {
                            "resources" => ModuleKind::Resources,
                            "data" => ModuleKind::Data,
                            "script" => ModuleKind::Script,
                            _ => ModuleKind::Other,
                        };
                        Module {
                            kind,
                            type_name,
                            uuid: str_of(m, "uuid"),
                            version: m
                                .get("version")
                                .and_then(Version::from_json)
                                .unwrap_or_default(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let dependencies = root
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|d| Dependency {
                        id: d
                            .get("uuid")
                            .or_else(|| d.get("module_name"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        version: d.get("version").and_then(Version::from_json),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let capabilities = root
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let subpacks = root
            .get("subpacks")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|s| Subpack {
                        folder_name: str_of(s, "folder_name"),
                        name: str_of(s, "name"),
                        memory_performance_tier: s
                            .get("memory_performance_tier")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as u32,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Manifest {
            format_version: root
                .get("format_version")
                .and_then(Value::as_u64)
                .unwrap_or(1) as u32,
            name: str_of(header, "name"),
            description: str_of(header, "description"),
            uuid,
            version: header
                .get("version")
                .and_then(Version::from_json)
                .unwrap_or_default(),
            min_engine_version: header
                .get("min_engine_version")
                .and_then(Version::from_json),
            modules,
            dependencies,
            capabilities,
            subpacks,
            settings: crate::settings::parse(root),
        })
    }

    /// Whether the pack asks for a newer game than the formats this crate knows.
    pub fn needs_newer_engine(&self) -> bool {
        self.min_engine_version.is_some_and(|v| v > ENGINE_VERSION) || self.format_version > 2
    }

    pub fn is_resource_pack(&self) -> bool {
        self.modules.iter().any(|m| m.kind == ModuleKind::Resources)
    }

    /// The subpack folder named `folder`.
    pub fn subpack(&self, folder: &str) -> Option<&Subpack> {
        self.subpacks.iter().find(|s| s.folder_name == folder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      // a resource pack with subpacks
      "format_version": 3,
      "header": { "name": "pack.name", "description": "pack.description",
                  "uuid": "2cf066eb-1254-4b7d-affb-80fe3216b18c", "version": "1.1.24",
                  "min_engine_version": [1, 21, 120] },
      "modules": [ { "type": "resources", "uuid": "22ed17a6", "version": "1.1.24" } ],
      "capabilities": ["pbr"],
      "subpacks": [ { "folder_name": "0_e", "memory_performance_tier": 1, "name": "Vanilla Experience" } ]
    }"#;

    #[test]
    fn reads_a_resource_manifest() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.format_version, 3);
        assert_eq!(m.version, Version([1, 1, 24]));
        assert_eq!(m.min_engine_version, Some(Version([1, 21, 120])));
        assert!(m.is_resource_pack());
        assert_eq!(m.capabilities, ["pbr"]);
        assert_eq!(m.subpack("0_e").unwrap().name, "Vanilla Experience");
        assert!(m.subpack("9_e").is_none());
    }

    #[test]
    fn module_kinds() {
        let m = Manifest::parse(
            r#"{"header":{"uuid":"u"},"modules":[{"type":"data"},{"type":"script"},{"type":"skin_pack"}]}"#,
        )
        .unwrap();
        let kinds: Vec<_> = m.modules.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            [ModuleKind::Data, ModuleKind::Script, ModuleKind::Other]
        );
        assert!(!m.is_resource_pack());
    }

    #[test]
    fn needs_a_uuid() {
        assert!(Manifest::parse(r#"{"header":{}}"#).is_err());
        assert!(Manifest::parse("{}").is_err());
    }
}
