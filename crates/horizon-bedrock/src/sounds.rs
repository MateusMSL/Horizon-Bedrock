//! `sounds/sound_definitions.json` and `sounds.json`: named sounds with their variants, and
//! the replacements a pack makes to entity sound events (`entity_sounds`).
//!
//! The game keeps its own sound logic: it asks [`EventSounds::entity_event`] what the pack
//! wants for an event it is about to play, and plays what comes back (or its own sound when the
//! pack says nothing).

use crate::molang::{Host, NoHost, Rng, Value, Vars};
use serde_json::Value as Json;
use std::collections::HashMap;

/// A number or a `[min, max]` range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range(pub f32, pub f32);

impl Range {
    fn from_json(v: Option<&Json>, default: f32) -> Range {
        match v {
            Some(Json::Number(n)) => {
                let n = n.as_f64().unwrap_or(default as f64) as f32;
                Range(n, n)
            }
            Some(Json::Array(a)) if a.len() >= 2 => Range(
                a[0].as_f64().unwrap_or(1.0) as f32,
                a[1].as_f64().unwrap_or(1.0) as f32,
            ),
            _ => Range(default, default),
        }
    }

    pub fn pick(self, rng: &mut Rng) -> f32 {
        self.0 + (self.1 - self.0) * rng.unit()
    }
}

/// One file a sound can play.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// Path in the pack without extension (`sounds/mob/x`).
    pub name: String,
    pub volume: Range,
    pub pitch: Range,
    pub weight: u32,
    pub is_3d: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub category: Option<String>,
    pub min_distance: Option<f32>,
    pub max_distance: Option<f32>,
    pub variants: Vec<Variant>,
}

/// What to play: a file, volume and pitch.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub name: String,
    pub volume: f32,
    pub pitch: f32,
    pub is_3d: bool,
}

impl Definition {
    pub fn pick(&self, rng: &mut Rng) -> Option<Pick> {
        let total: u32 = self.variants.iter().map(|v| v.weight.max(1)).sum();
        if total == 0 {
            return None;
        }
        let mut at = (rng.unit() * total as f32) as u32;
        for v in &self.variants {
            let w = v.weight.max(1);
            if at < w {
                return Some(Pick {
                    name: v.name.clone(),
                    volume: v.volume.pick(rng),
                    pitch: v.pitch.pick(rng),
                    is_3d: v.is_3d,
                });
            }
            at -= w;
        }
        None
    }
}

/// Every sound definition of a pack, by lower-case name.
#[derive(Debug, Default)]
pub struct Definitions {
    pub map: HashMap<String, Definition>,
}

impl Definitions {
    pub fn add_file(&mut self, root: &Json) {
        let Some(o) = root.get("sound_definitions").and_then(Json::as_object) else {
            return;
        };
        for (name, d) in o {
            let Some(list) = d.get("sounds").and_then(Json::as_array) else {
                continue;
            };
            let variants = list
                .iter()
                .filter_map(|s| match s {
                    Json::String(n) => Some(Variant {
                        name: n.clone(),
                        volume: Range(1.0, 1.0),
                        pitch: Range(1.0, 1.0),
                        weight: 1,
                        is_3d: true,
                    }),
                    Json::Object(o) => Some(Variant {
                        name: o.get("name")?.as_str()?.to_owned(),
                        volume: Range::from_json(o.get("volume"), 1.0),
                        pitch: Range::from_json(o.get("pitch"), 1.0),
                        weight: o.get("weight").and_then(Json::as_u64).unwrap_or(1) as u32,
                        is_3d: o.get("is3D").and_then(Json::as_bool).unwrap_or(true),
                    }),
                    _ => None,
                })
                .collect();
            self.map.insert(
                name.to_ascii_lowercase(),
                Definition {
                    category: d.get("category").and_then(Json::as_str).map(str::to_owned),
                    min_distance: d
                        .get("min_distance")
                        .and_then(Json::as_f64)
                        .map(|n| n as f32),
                    max_distance: d
                        .get("max_distance")
                        .and_then(Json::as_f64)
                        .map(|n| n as f32),
                    variants,
                },
            );
        }
    }

    pub fn get(&self, name: &str) -> Option<&Definition> {
        self.map.get(&name.to_ascii_lowercase())
    }

    /// Picks a variant of the definition `name`.
    pub fn pick(&self, name: &str, rng: &mut Rng) -> Option<Pick> {
        self.get(name)?.pick(rng)
    }
}

/// What a pack says about an event.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The pack silences it.
    Silent,
    /// The pack plays a definition.
    Play {
        sound: String,
        volume: Range,
        pitch: Range,
    },
}

#[derive(Debug, Default)]
struct EntitySounds {
    events: HashMap<String, Event>,
    variant_key: Option<crate::molang::Program>,
    variants: HashMap<String, HashMap<String, Event>>,
}

/// The event replacements of `sounds.json`.
#[derive(Debug, Default)]
pub struct EventSounds {
    entities: HashMap<String, EntitySounds>,
    individual: HashMap<String, Event>,
}

fn event(v: &Json) -> Option<Event> {
    match v {
        Json::String(s) if s.trim().is_empty() => Some(Event::Silent),
        Json::String(s) => Some(Event::Play {
            sound: s.clone(),
            volume: Range(1.0, 1.0),
            pitch: Range(1.0, 1.0),
        }),
        Json::Object(o) => {
            let sound = o.get("sound").and_then(Json::as_str).unwrap_or("");
            if sound.trim().is_empty() {
                return Some(Event::Silent);
            }
            Some(Event::Play {
                sound: sound.to_owned(),
                volume: Range::from_json(o.get("volume"), 1.0),
                pitch: Range::from_json(o.get("pitch"), 1.0),
            })
        }
        _ => None,
    }
}

fn events(v: Option<&Json>) -> HashMap<String, Event> {
    v.and_then(Json::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.to_ascii_lowercase(), event(v)?)))
                .collect()
        })
        .unwrap_or_default()
}

impl EventSounds {
    pub fn add_file(&mut self, root: &Json) {
        if let Some(o) = root
            .pointer("/entity_sounds/entities")
            .and_then(Json::as_object)
        {
            for (name, e) in o {
                let mut s = EntitySounds {
                    events: events(e.get("events")),
                    ..Default::default()
                };
                if let Some(v) = e.get("variants") {
                    s.variant_key = v
                        .get("key")
                        .and_then(Json::as_str)
                        .and_then(crate::animation::compile);
                    if let Some(m) = v.get("map").and_then(Json::as_object) {
                        for (k, body) in m {
                            s.variants
                                .insert(k.to_ascii_lowercase(), events(body.get("events")));
                        }
                    }
                }
                self.entities.insert(strip_namespace(name), s);
            }
        }
        self.individual
            .extend(events(root.pointer("/individual_event_sounds/events")));
    }

    /// The pack's answer for `event` on `entity` (`minecraft:zombie` or `zombie`), or `None`
    /// when the pack leaves it alone. The variant key is evaluated against `host`.
    pub fn entity_event(
        &self,
        entity: &str,
        event: &str,
        host: &mut dyn Host,
        vars: &mut Vars,
        rng: &mut Rng,
    ) -> Option<Event> {
        let s = self.entities.get(&strip_namespace(entity))?;
        let event = event.to_ascii_lowercase();
        if let Some(key) = &s.variant_key {
            let value =
                crate::animation::Env { host, vars, rng }.run(key, Default::default(), Value::Null);
            if let Value::Str(k) = value
                && let Some(e) = s
                    .variants
                    .get(&k.to_ascii_lowercase())
                    .and_then(|m| m.get(&event))
            {
                return Some(e.clone());
            }
        }
        s.events.get(&event).cloned()
    }

    /// Same, without a variant key to evaluate.
    pub fn entity_event_plain(&self, entity: &str, event: &str) -> Option<Event> {
        self.entity_event(
            entity,
            event,
            &mut NoHost,
            &mut Vars::default(),
            &mut Rng::new(1),
        )
    }

    /// Events not tied to an entity (`ignite`).
    pub fn individual(&self, event: &str) -> Option<&Event> {
        self.individual.get(&event.to_ascii_lowercase())
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.individual.is_empty()
    }
}

fn strip_namespace(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc;

    #[test]
    fn definitions_pick_by_weight_and_range() {
        let mut d = Definitions::default();
        d.add_file(
            &jsonc::parse(
                r#"{"sound_definitions":{"Pack:Hit":{"category":"neutral","max_distance":16,"sounds":[
                  "sounds/a", {"name":"sounds/b","volume":0.5,"pitch":[0.8,1.2],"weight":3,"is3D":false}]},
                  "pack:empty":{"sounds":[]}}}"#,
            )
            .unwrap(),
        );
        let def = d.get("pack:hit").unwrap();
        assert_eq!(def.max_distance, Some(16.0));
        assert_eq!(def.variants.len(), 2);
        let mut rng = Rng::new(7);
        let (mut a, mut b) = (0, 0);
        for _ in 0..400 {
            let p = d.pick("PACK:HIT", &mut rng).unwrap();
            if p.name == "sounds/a" {
                a += 1;
            } else {
                b += 1;
                assert_eq!(p.volume, 0.5);
                assert!((0.8..=1.2).contains(&p.pitch) && !p.is_3d);
            }
        }
        assert!(b > a * 2, "weight 3 against 1: {a} {b}");
        assert!(d.pick("pack:empty", &mut rng).is_none());
        assert!(d.pick("missing", &mut rng).is_none());
    }

    struct H(String);
    impl Host for H {
        fn query(&mut self, name: &str, _args: &[Value]) -> Value {
            if name == "weapon" {
                Value::Str(self.0.as_str().into())
            } else {
                Value::Null
            }
        }
    }

    #[test]
    fn entity_events_with_variants() {
        let mut e = EventSounds::default();
        e.add_file(
            &jsonc::parse(
                r#"{"entity_sounds":{"entities":{"drowned":{
                  "events":{"step":""},
                  "variants":{"key":"q.weapon","map":{"iron":{"events":{"attack":{"sound":"p:slash","volume":1,"pitch":[0.9,1.1]}}}}}}}},
                  "individual_event_sounds":{"events":{"ignite":{"sound":"p:fire","pitch":[0.8,1.2]}}}}"#,
            )
            .unwrap(),
        );
        let (mut vars, mut rng) = (Vars::default(), Rng::new(1));
        let mut h = H("iron".into());
        assert_eq!(
            e.entity_event("minecraft:drowned", "step", &mut h, &mut vars, &mut rng),
            Some(Event::Silent)
        );
        match e.entity_event("drowned", "ATTACK", &mut h, &mut vars, &mut rng) {
            Some(Event::Play { sound, pitch, .. }) => {
                assert_eq!((sound.as_str(), pitch), ("p:slash", Range(0.9, 1.1)))
            }
            other => panic!("{other:?}"),
        }
        let mut other = H("stone".into());
        assert_eq!(
            e.entity_event("drowned", "attack", &mut other, &mut vars, &mut rng),
            None
        );
        assert_eq!(
            e.entity_event("zombie", "step", &mut h, &mut vars, &mut rng),
            None
        );
        assert_eq!(e.entity_event_plain("drowned", "step"), Some(Event::Silent));
        assert!(matches!(e.individual("Ignite"), Some(Event::Play { .. })));
    }
}
