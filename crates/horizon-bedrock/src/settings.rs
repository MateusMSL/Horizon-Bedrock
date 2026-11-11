//! A pack's own options: the manifest's `settings` (toggle, dropdown, multiselect, slider),
//! the values a player chose, and the answers to `query.is_pack_setting_enabled` and
//! `query.is_pack_setting_selected`.
//!
//! Definitions travel with the converted pack (`pack_settings.json` in the pack's own folder, labels already looked up in
//! the pack's language file). The values live in the game folder's `config/horizon-addons.json`,
//! by pack file name, beside the subpack choice:
//!
//! ```json
//! { "Pack.mcpack": { "subpack": "SP2", "settings": { "ns:flag": true, "ns:mode": "fast" } } }
//! ```

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// The file the converter writes next to the pack's other data.
pub const FILE: &str = "pack_settings.json";
/// The settings file, relative to the game folder.
pub const CONFIG: &str = "config/horizon-addons.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Toggle,
    Dropdown,
    Multiselect,
    Slider,
}

/// What a setting holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Flag(bool),
    One(String),
    Many(Vec<String>),
    Number(f64),
}

impl Choice {
    fn to_json(&self) -> Value {
        match self {
            Choice::Flag(b) => json!(b),
            Choice::One(s) => json!(s),
            Choice::Many(v) => json!(v),
            Choice::Number(n) => json!(n),
        }
    }

    fn from_json(v: &Value) -> Option<Choice> {
        Some(match v {
            Value::Bool(b) => Choice::Flag(*b),
            Value::String(s) => Choice::One(s.clone()),
            Value::Number(n) => Choice::Number(n.as_f64()?),
            Value::Array(a) => Choice::Many(
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect(),
            ),
            _ => return None,
        })
    }
}

/// One setting of a pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    /// `namespace:name`, what the expressions name.
    pub name: String,
    /// The label, or its translation key until [`Def::translate`] looks it up.
    pub label: String,
    pub kind: Kind,
    /// Dropdown and multiselect options: name and label.
    pub options: Vec<(String, String)>,
    pub default: Choice,
    /// Slider bounds and step.
    pub range: Option<(f64, f64, f64)>,
}

impl Def {
    /// Looks the label and the options' labels up in a language file.
    pub fn translate(&mut self, lang: &BTreeMap<String, String>) {
        let find = |k: &str| lang.get(k).cloned().unwrap_or_else(|| k.to_owned());
        self.label = find(&self.label);
        for (_, label) in &mut self.options {
            *label = find(label);
        }
    }

    /// The label without the grey explanation Bedrock appends after `§8`.
    pub fn title(&self) -> &str {
        title_of(&self.label)
    }

    fn to_json(&self) -> Value {
        let kind = match self.kind {
            Kind::Toggle => "toggle",
            Kind::Dropdown => "dropdown",
            Kind::Multiselect => "multiselect",
            Kind::Slider => "slider",
        };
        let mut o = Map::new();
        o.insert("type".into(), json!(kind));
        o.insert("name".into(), json!(self.name));
        o.insert("text".into(), json!(self.label));
        o.insert(
            "options".into(),
            Value::Array(
                self.options
                    .iter()
                    .map(|(n, t)| json!({"name": n, "text": t}))
                    .collect(),
            ),
        );
        o.insert("default".into(), self.default.to_json());
        if let Some((lo, hi, step)) = self.range {
            o.insert("min".into(), json!(lo));
            o.insert("max".into(), json!(hi));
            o.insert("step".into(), json!(step));
        }
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Option<Def> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let kind = match v.get("type")?.as_str()? {
            "toggle" => Kind::Toggle,
            "dropdown" => Kind::Dropdown,
            "multiselect" => Kind::Multiselect,
            "slider" => Kind::Slider,
            _ => return None,
        };
        let name = s("name")?;
        let options: Vec<(String, String)> = v
            .get("options")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        let n = o.get("name")?.as_str()?.to_owned();
                        let t = o
                            .get("text")
                            .and_then(Value::as_str)
                            .map_or_else(|| n.clone(), str::to_owned);
                        Some((n, t))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let given = v
            .get("default")
            .or_else(|| v.get("defaults"))
            .and_then(Choice::from_json);
        let default = match (kind, given) {
            (Kind::Toggle, Some(c @ Choice::Flag(_))) => c,
            (Kind::Toggle, _) => Choice::Flag(false),
            (Kind::Dropdown, Some(c @ Choice::One(_))) => c,
            (Kind::Dropdown, _) => Choice::One(options.first().map(|o| o.0.clone())?),
            (Kind::Multiselect, Some(c @ Choice::Many(_))) => c,
            (Kind::Multiselect, _) => Choice::Many(Vec::new()),
            (Kind::Slider, Some(c @ Choice::Number(_))) => c,
            (Kind::Slider, _) => {
                Choice::Number(v.get("min").and_then(Value::as_f64).unwrap_or(0.0))
            }
        };
        let num = |k: &str| v.get(k).and_then(Value::as_f64);
        let range = (kind == Kind::Slider)
            .then(|| {
                Some((
                    num("min")?,
                    num("max")?,
                    num("step").unwrap_or(1.0).max(f64::EPSILON),
                ))
            })
            .flatten();
        Some(Def {
            name,
            label: s("text").unwrap_or_default(),
            kind,
            options,
            default,
            range,
        })
    }

    /// The choice after the player pressed the setting once: a toggle flips, a dropdown goes to
    /// the next option (and round), a slider steps up (and round). A multiselect option is
    /// flipped with [`Def::flip`] instead.
    pub fn press(&self, now: &Choice) -> Choice {
        match (self.kind, now) {
            (Kind::Toggle, Choice::Flag(b)) => Choice::Flag(!b),
            (Kind::Dropdown, Choice::One(cur)) => {
                let at = self.options.iter().position(|o| &o.0 == cur);
                let next = at.map_or(0, |i| (i + 1) % self.options.len().max(1));
                self.options
                    .get(next)
                    .map_or_else(|| now.clone(), |o| Choice::One(o.0.clone()))
            }
            (Kind::Slider, Choice::Number(n)) => match self.range {
                Some((lo, hi, step)) => {
                    let next = n + step;
                    Choice::Number(if next > hi + 1e-9 { lo } else { next })
                }
                None => now.clone(),
            },
            _ => now.clone(),
        }
    }

    /// A multiselect with `option` turned on if it was off and off if it was on.
    pub fn flip(&self, now: &Choice, option: &str) -> Choice {
        let mut have = match now {
            Choice::Many(v) => v.clone(),
            _ => Vec::new(),
        };
        if let Some(at) = have.iter().position(|o| o == option) {
            have.remove(at);
        } else {
            have.push(option.to_owned());
        }
        // Keep the manifest's order.
        have.sort_by_key(|o| self.options.iter().position(|p| &p.0 == o));
        Choice::Many(have)
    }
}

pub fn title_of(label: &str) -> &str {
    label.split("\u{a7}8").next().unwrap_or(label).trim()
}

/// The text after the title, the explanation.
pub fn detail_of(label: &str) -> &str {
    label.split_once("\u{a7}8").map_or("", |(_, d)| d.trim())
}

/// The settings in a manifest's `settings` array (labels still keys).
pub fn parse(manifest: &Value) -> Vec<Def> {
    manifest
        .get("settings")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Def::from_json).collect())
        .unwrap_or_default()
}

/// The settings of `pack` with their labels looked up in its language file (`en_US`, else the
/// first one there is).
pub fn of_pack(pack: &crate::pack::Pack) -> Vec<Def> {
    let mut defs = pack.manifest().settings.clone();
    if defs.is_empty() {
        return defs;
    }
    let file = ["texts/en_US.lang", "texts/en_GB.lang"]
        .into_iter()
        .find(|p| pack.contains(p))
        .or_else(|| pack.paths_in("texts").find(|p| p.ends_with(".lang")));
    let lang = file
        .and_then(|f| pack.read(f).ok())
        .map(|b| crate::lang::parse(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    for d in &mut defs {
        d.translate(&lang);
    }
    defs
}

/// The definitions as the converted pack stores them.
pub fn to_json(defs: &[Def]) -> Value {
    Value::Array(defs.iter().map(Def::to_json).collect())
}

/// Definitions read back from [`to_json`]'s form.
pub fn from_json(v: &Value) -> Vec<Def> {
    v.as_array()
        .map(|a| a.iter().filter_map(Def::from_json).collect())
        .unwrap_or_default()
}

/// The game folder's settings file.
pub fn config_path(game: &Path) -> PathBuf {
    game.join(CONFIG)
}

/// Every pack's saved options, by pack file name.
#[derive(Debug, Default, Clone)]
pub struct Config {
    packs: BTreeMap<String, Value>,
}

impl Config {
    pub fn read(game: &Path) -> Config {
        let packs = std::fs::read(config_path(game))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Config { packs }
    }

    pub fn write(&self, game: &Path) -> std::io::Result<()> {
        let path = config_path(game);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&self.packs).map_err(std::io::Error::other)?,
        )
    }

    pub fn subpack(&self, pack: &str) -> Option<String> {
        self.packs
            .get(pack)?
            .get("subpack")?
            .as_str()
            .map(str::to_owned)
    }

    pub fn set_subpack(&mut self, pack: &str, subpack: Option<&str>) {
        let entry = self.entry(pack);
        match subpack {
            Some(s) => {
                entry.insert("subpack".into(), json!(s));
            }
            None => {
                entry.remove("subpack");
            }
        }
        self.tidy(pack);
    }

    pub fn choice(&self, pack: &str, name: &str) -> Option<Choice> {
        Choice::from_json(self.packs.get(pack)?.get("settings")?.get(name)?)
    }

    pub fn set_choice(&mut self, pack: &str, name: &str, choice: &Choice) {
        let entry = self.entry(pack);
        let settings = entry
            .entry("settings")
            .or_insert_with(|| json!({}))
            .as_object_mut();
        if let Some(settings) = settings {
            settings.insert(name.to_owned(), choice.to_json());
        }
    }

    /// Every saved value of every pack, by setting name.
    pub fn choices(&self) -> HashMap<String, Choice> {
        let mut out = HashMap::new();
        for pack in self.packs.values() {
            if let Some(o) = pack.get("settings").and_then(Value::as_object) {
                for (k, v) in o {
                    if let Some(c) = Choice::from_json(v) {
                        out.insert(k.clone(), c);
                    }
                }
            }
        }
        out
    }

    fn entry(&mut self, pack: &str) -> &mut Map<String, Value> {
        let v = self
            .packs
            .entry(pack.to_owned())
            .or_insert_with(|| json!({}));
        if !v.is_object() {
            *v = json!({});
        }
        v.as_object_mut().expect("an object")
    }

    fn tidy(&mut self, pack: &str) {
        if self
            .packs
            .get(pack)
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty)
        {
            self.packs.remove(pack);
        }
    }
}

/// What the expressions are answered from: the definitions' defaults under the chosen values.
#[derive(Debug, Default)]
struct Active {
    values: HashMap<String, Choice>,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

/// Makes `defs` with `chosen` laid over their defaults what [`is_enabled`] and [`is_selected`]
/// answer from.
pub fn install(defs: &[Def], chosen: &HashMap<String, Choice>) {
    let mut values: HashMap<String, Choice> = defs
        .iter()
        .map(|d| (d.name.clone(), d.default.clone()))
        .collect();
    for d in defs {
        if let Some(c) = chosen.get(&d.name) {
            // A value of another kind (an old file) is not taken.
            if std::mem::discriminant(c) == std::mem::discriminant(&d.default) {
                values.insert(d.name.clone(), c.clone());
            }
        }
    }
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(Active { values });
}

/// The value now (default or chosen) of the setting `name`.
pub fn value(name: &str) -> Option<Choice> {
    ACTIVE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()?
        .values
        .get(name)
        .cloned()
}

/// `query.is_pack_setting_enabled('name')`: a toggle that is on.
pub fn is_enabled(name: &str) -> bool {
    matches!(value(name), Some(Choice::Flag(true)))
}

/// `query.is_pack_setting_selected('name', 'option')`: the dropdown shows `option`, or the
/// multiselect has it on.
pub fn is_selected(name: &str, option: &str) -> bool {
    match value(name) {
        Some(Choice::One(o)) => o == option,
        Some(Choice::Many(v)) => v.iter().any(|o| o == option),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Value {
        json!({"settings": [
            {"type": "label", "text": "Just text"},
            {"type": "toggle", "name": "ns:on", "text": "k.on", "default": true},
            {"type": "dropdown", "name": "ns:mode", "text": "k.mode", "default": "b",
             "options": [{"name": "a", "text": "k.a"}, {"name": "b", "text": "k.b"}]},
            {"type": "multiselect", "name": "ns:many", "text": "k.many", "defaults": ["x"],
             "options": [{"name": "x"}, {"name": "y"}, {"name": "z"}]},
            {"type": "slider", "name": "ns:level", "text": "k.level", "min": 1, "max": 3, "step": 1, "default": 2}
        ]})
    }

    #[test]
    fn the_manifest_gives_every_kind_with_its_default() {
        let defs = parse(&manifest());
        assert_eq!(defs.len(), 4);
        assert_eq!(defs[0].default, Choice::Flag(true));
        assert_eq!(defs[1].default, Choice::One("b".into()));
        assert_eq!(defs[2].default, Choice::Many(vec!["x".into()]));
        assert_eq!(defs[3].default, Choice::Number(2.0));
        assert_eq!(defs[3].range, Some((1.0, 3.0, 1.0)));
        assert_eq!(from_json(&to_json(&defs)), defs);
    }

    #[test]
    fn labels_come_from_the_language_file_and_lose_the_grey_detail() {
        let mut defs = parse(&manifest());
        let lang = crate::lang::parse("k.on=Armour \u{a7}8Shows it\nk.b=Bee\n");
        for d in &mut defs {
            d.translate(&lang);
        }
        assert_eq!(defs[0].title(), "Armour");
        assert_eq!(detail_of(&defs[0].label), "Shows it");
        assert_eq!(defs[1].options[1].1, "Bee");
        assert_eq!(defs[1].label, "k.mode");
    }

    #[test]
    fn pressing_steps_through_the_options() {
        let defs = parse(&manifest());
        assert_eq!(defs[0].press(&Choice::Flag(true)), Choice::Flag(false));
        assert_eq!(
            defs[1].press(&Choice::One("a".into())),
            Choice::One("b".into())
        );
        assert_eq!(
            defs[1].press(&Choice::One("b".into())),
            Choice::One("a".into())
        );
        assert_eq!(defs[3].press(&Choice::Number(3.0)), Choice::Number(1.0));
        assert_eq!(
            defs[2].flip(&Choice::Many(vec!["x".into()]), "z"),
            Choice::Many(vec!["x".into(), "z".into()])
        );
        assert_eq!(
            defs[2].flip(&Choice::Many(vec!["x".into(), "z".into()]), "x"),
            Choice::Many(vec!["z".into()])
        );
    }

    #[test]
    fn the_expressions_see_defaults_under_the_chosen_values() {
        let defs = parse(&manifest());
        let mut chosen = HashMap::new();
        chosen.insert("ns:on".to_owned(), Choice::Flag(false));
        chosen.insert("ns:mode".to_owned(), Choice::Flag(true)); // wrong kind: ignored
        chosen.insert("ns:many".to_owned(), Choice::Many(vec!["y".into()]));
        install(&defs, &chosen);
        assert!(!is_enabled("ns:on"));
        assert!(is_selected("ns:mode", "b"));
        assert!(is_selected("ns:many", "y") && !is_selected("ns:many", "x"));
        assert!(!is_enabled("ns:nothing") && !is_selected("ns:nothing", "a"));
    }

    #[test]
    fn the_config_keeps_the_subpack_and_the_values_by_pack() {
        let dir = std::env::temp_dir().join(format!("horizon-bedrock-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = Config::read(&dir);
        assert_eq!(c.subpack("P.mcpack"), None);
        c.set_subpack("P.mcpack", Some("SP2"));
        c.set_choice("P.mcpack", "ns:on", &Choice::Flag(false));
        c.write(&dir).unwrap();
        let c = Config::read(&dir);
        assert_eq!(c.subpack("P.mcpack").as_deref(), Some("SP2"));
        assert_eq!(c.choice("P.mcpack", "ns:on"), Some(Choice::Flag(false)));
        assert_eq!(c.choices()["ns:on"], Choice::Flag(false));
        let mut c = c;
        c.set_subpack("P.mcpack", None);
        assert_eq!(c.subpack("P.mcpack"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
