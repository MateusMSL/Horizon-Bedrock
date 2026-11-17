//! What a mob is drawn with (`LivingEntityRenderer.getTextureLocation` and the render layers'
//! textures): its body texture by variant, age and state, its colour, and the extra layers over it
//! (wool, eyes, outer layers, overlays, collars, armour).

use super::*;

/// The model layer key and texture for the mob's body.
pub fn body(m: &Mob) -> (Cow<'static, str>, Cow<'_, str>) {
    use Species as S;
    let baby = m.is_baby();
    let b = |s: &str| -> Cow<'static, str> {
        if baby {
            interned(format_args!("{s}_baby"))
        } else {
            interned(format_args!("{s}"))
        }
    };
    if avatar(m) {
        // AvatarRenderer: PlayerModel's slim layer for a slim skin.
        return (
            if m.slim { "player_slim#main" } else { "main" }.into(),
            m.skin.as_deref().unwrap_or(&m.kind.texture).into(),
        );
    }
    match m.species() {
        S::Llama | S::TraderLlama => {
            let name = if m.species() == S::Llama {
                "llama"
            } else {
                "trader_llama"
            };
            (
                baby_or(
                    m,
                    interned(format_args!("{name}#main")),
                    interned(format_args!("{name}_baby#main")),
                ),
                super::super::llama::texture(m).into(),
            )
        }
        s if super::super::equine::is_equine(s) => {
            let name = super::super::equine::model_name(s);
            (
                baby_or(
                    m,
                    interned(format_args!("{name}#main")),
                    interned(format_args!("{name}_baby#main")),
                ),
                super::super::equine::texture(m).into(),
            )
        }
        S::Wolf => (
            baby_or(m, "wolf#main", "wolf_baby#main"),
            super::super::tame::texture(m).into(),
        ),
        S::Panda => (
            baby_or(m, "panda#main", "panda_baby#main"),
            super::super::panda::texture(m).into(),
        ),
        S::HappyGhast => (
            baby_or(m, "happy_ghast#main", "happy_ghast_baby#main"),
            if baby {
                "entity/ghast/happy_ghast_baby".into()
            } else {
                "entity/ghast/happy_ghast".into()
            },
        ),
        S::Nautilus | S::ZombieNautilus => (
            super::super::nautilus::model(m).into(),
            super::super::nautilus::texture(m).into(),
        ),
        S::SulfurCube => {
            let (model, texture) = super::super::sulfur_cube::body(m);
            (model.into(), texture.into())
        }
        S::Sniffer => (
            baby_or(m, "sniffer#main", "sniffer_baby#main"),
            if baby {
                "entity/sniffer/snifflet".into()
            } else {
                "entity/sniffer/sniffer".into()
            },
        ),
        S::Shulker => (
            "shulker#main".into(),
            super::super::shulker::texture(m).into(),
        ),
        S::Vex => ("vex#main".into(), super::super::vex::texture(m).into()),
        S::Wither => (
            "wither#main".into(),
            super::super::wither::texture(m).into(),
        ),
        S::Ghast => (
            "ghast#main".into(),
            if m.extra.flag {
                "entity/ghast/ghast_shooting".into()
            } else {
                "entity/ghast/ghast".into()
            },
        ),
        S::Cat => (
            baby_or(m, "cat#main", "cat_baby#main"),
            super::super::tame::cat_texture(m).into(),
        ),
        S::Frog => (
            "frog#main".into(),
            interned(format_args!("entity/frog/frog_{}", climate(m))),
        ),
        // RabbitRenderer.getTextureLocation: the Toast name tag, else the
        // variant (the killer bunny's id is 99).
        S::Rabbit => {
            let name = if m.custom_name.as_deref() == Some("Toast") {
                "toast"
            } else {
                match m.extra.variant {
                    1 => "white",
                    2 => "black",
                    3 => "white_splotched",
                    4 => "gold",
                    5 => "salt",
                    99 => "caerbannog",
                    _ => "brown",
                }
            };
            (
                baby_or(m, "rabbit#main", "rabbit_baby#main"),
                b(&interned(format_args!("entity/rabbit/rabbit_{name}"))),
            )
        }
        // ParrotRenderer.getVariantTexture.
        S::Parrot => {
            let name = ["red_blue", "blue", "green", "yellow_blue", "grey"]
                [usize::from(m.extra.variant).min(4)];
            (
                m.kind.model_name().into(),
                interned(format_args!("entity/parrot/parrot_{name}")),
            )
        }
        // AxolotlRenderer.getTextureLocation.
        S::Axolotl => {
            let name =
                ["lucy", "wild", "gold", "cyan", "blue"][usize::from(m.extra.variant).min(4)];
            (
                baby_or(m, "axolotl#main", "axolotl_baby#main"),
                b(&interned(format_args!("entity/axolotl/axolotl_{name}"))),
            )
        }
        // SalmonRenderer.submit: the model of the salmon's size.
        S::Salmon => {
            let model = match m.extra.variant {
                0 => "salmon_small#main",
                2 => "salmon_large#main",
                _ => "salmon#main",
            };
            (model.into(), "entity/fish/salmon".into())
        }
        // The baby models and textures of AgeableMobRenderer's other
        // subclasses.
        S::Goat => (
            baby_or(m, "goat#main", "goat_baby#main"),
            b(&m.kind.texture),
        ),
        S::PolarBear => (
            baby_or(m, "polar_bear#main", "polar_bear_baby#main"),
            b(&m.kind.texture),
        ),
        S::Ocelot => (
            baby_or(m, "ocelot#main", "ocelot_baby#main"),
            b(&m.kind.texture),
        ),
        S::Turtle => (
            baby_or(m, "turtle#main", "turtle_baby#main"),
            b(&m.kind.texture),
        ),
        S::Armadillo => (
            baby_or(m, "armadillo#main", "armadillo_baby#main"),
            b(&m.kind.texture),
        ),
        // FoxRenderer.getTextureLocation: snow variant, sleeping, baby.
        S::Fox => (
            baby_or(m, "fox#main", "fox_baby#main"),
            interned(format_args!(
                "entity/fox/fox{}{}{}",
                if m.extra.variant == 0 { "" } else { "_snow" },
                if super::super::fox::flag(m, super::super::fox::SLEEPING) {
                    "_sleep"
                } else {
                    ""
                },
                if baby { "_baby" } else { "" },
            )),
        ),
        S::Pig => {
            let model = if baby {
                "pig_baby#main".into()
            } else if m.extra.variant == 2 {
                "cold_pig#main".into()
            } else {
                "pig#main".into()
            };
            (
                model,
                b(&interned(format_args!("entity/pig/pig_{}", climate(m)))),
            )
        }
        S::Cow => {
            let model = match (m.extra.variant, baby) {
                (1, false) => "warm_cow#main",
                (1, true) => "warm_cow_baby#main",
                (2, false) => "cold_cow#main",
                (2, true) => "cold_cow_baby#main",
                (_, true) => "cow_baby#main",
                _ => "cow#main",
            };
            (
                model.into(),
                b(&interned(format_args!("entity/cow/cow_{}", climate(m)))),
            )
        }
        S::Mooshroom => (
            baby_or(m, "mooshroom#main", "mooshroom_baby#main"),
            b(if m.extra.variant == 1 {
                "entity/cow/mooshroom_brown"
            } else {
                "entity/cow/mooshroom_red"
            }),
        ),
        S::Chicken => {
            let model = if baby {
                "chicken_baby#main"
            } else if m.extra.variant == 2 {
                "cold_chicken#main"
            } else {
                "chicken#main"
            };
            (
                model.into(),
                b(&interned(format_args!(
                    "entity/chicken/chicken_{}",
                    climate(m)
                ))),
            )
        }
        S::Sheep => (
            baby_or(m, "sheep#main", "sheep_baby#main"),
            b("entity/sheep/sheep"),
        ),
        S::Zombie => (
            baby_or(m, "zombie#main", "zombie_baby#main"),
            b("entity/zombie/zombie"),
        ),
        S::Husk => (
            baby_or(m, "husk#main", "husk_baby#main"),
            b("entity/zombie/husk"),
        ),
        S::Drowned => (
            baby_or(m, "drowned#main", "drowned_baby#main"),
            b("entity/zombie/drowned"),
        ),
        S::ZombieVillager => (
            baby_or(m, "zombie_villager#main", "zombie_villager_baby#main"),
            b("entity/zombie_villager/zombie_villager"),
        ),
        S::Villager => (
            baby_or(m, "villager#main", "villager_baby#main"),
            b("entity/villager/villager"),
        ),
        S::Piglin => (
            baby_or(m, "piglin#main", "piglin_baby#main"),
            b("entity/piglin/piglin"),
        ),
        S::ZombifiedPiglin => (
            baby_or(m, "zombified_piglin#main", "zombified_piglin_baby#main"),
            b("entity/piglin/zombified_piglin"),
        ),
        S::Hoglin => (
            baby_or(m, "hoglin#main", "hoglin_baby#main"),
            b("entity/hoglin/hoglin"),
        ),
        S::Zoglin => (
            baby_or(m, "zoglin#main", "zoglin_baby#main"),
            b("entity/hoglin/zoglin"),
        ),
        S::Strider => (
            baby_or(m, "strider#main", "strider_baby#main"),
            b(if m.extra.flag {
                "entity/strider/strider_cold"
            } else {
                "entity/strider/strider"
            }),
        ),
        // BeeRenderer.getTextureLocation: angry, carrying nectar, or both.
        S::Bee => {
            let angry = super::super::bee::get(m).is_some_and(|b| b.anger_end > 0);
            let nectar = super::super::bee::has_nectar(m);
            let texture = match (angry, nectar) {
                (true, true) => "entity/bee/bee_angry_nectar",
                (true, false) => "entity/bee/bee_angry",
                (false, true) => "entity/bee/bee_nectar",
                (false, false) => "entity/bee/bee",
            };
            let layer = baby_or(m, "bee#main", "bee_baby#main");
            let texture = if layer.starts_with("bee_baby") {
                b(texture)
            } else {
                texture.into()
            };
            (layer, texture)
        }
        S::Squid => (
            baby_or(m, "squid#main", "squid_baby#main"),
            b("entity/squid/squid"),
        ),
        S::GlowSquid => (
            baby_or(m, "glow_squid#main", "glow_squid_baby#main"),
            b("entity/squid/glow_squid"),
        ),
        S::Dolphin => (
            baby_or(m, "dolphin#main", "dolphin_baby#main"),
            b("entity/dolphin/dolphin"),
        ),
        S::Pufferfish => {
            let model = match m.extra.size {
                0 => "pufferfish_small#main",
                1 => "pufferfish_medium#main",
                _ => "pufferfish_big#main",
            };
            (model.into(), "entity/fish/pufferfish".into())
        }
        S::TropicalFish => {
            if m.extra.variant < 6 {
                (
                    "tropical_fish_small#main".into(),
                    "entity/fish/tropical_a".into(),
                )
            } else {
                (
                    "tropical_fish_large#main".into(),
                    "entity/fish/tropical_b".into(),
                )
            }
        }
        // CopperGolemOxidationLevels: the texture of the weathering.
        S::CopperGolem => (
            m.kind.model_name().into(),
            super::super::copper_golem::texture(m).into(),
        ),
        _ => (m.kind.model_name().into(), m.kind.texture.as_str().into()),
    }
}
/// LivingEntityRenderer.getModelTint (tropical fish base colour).
pub fn body_color(m: &Mob, alpha: f32) -> u32 {
    match m.species() {
        Species::Wolf => super::super::tame::tint(m, alpha),
        Species::TropicalFish => abgr(dye(m.extra.variant2 as usize)),
        // LivingEntityRenderer.submit: a spectator is invisible
        // (Player.updateInvisibilityStatus) but not to another spectator or
        // itself, so its body is forced translucent (654311423, alpha 38).
        _ if avatar(m) && m.extra.spectator => 0x26FF_FFFF,
        _ => 0xFFFF_FFFF,
    }
}
/// Extra render layers drawn over the body with the same pose. `age` is the
/// render state's `ageInTicks` (`Mob.age` plus the partial tick).
pub fn layers(m: &Mob, age: f32) -> Vec<Layer> {
    use Species as S;
    let mut v = Vec::new();
    if avatar(m) {
        return v;
    }
    let white = 0xFFFF_FFFF;
    fn push(
        v: &mut Vec<Layer>,
        model: impl Into<Cow<'static, str>>,
        texture: impl Into<Cow<'static, str>>,
        color: u32,
        full_bright: bool,
    ) {
        v.push(Layer {
            model: model.into(),
            texture: texture.into(),
            color,
            full_bright,
            scroll: 0.0,
        })
    }
    match m.species() {
        // WolfArmorLayer, then WolfCollarLayer.
        S::Wolf if m.extra.tame => {
            let (model, texture) = if m.is_baby() {
                ("wolf_baby#main", "entity/wolf/wolf_collar_baby")
            } else {
                ("wolf#main", "entity/wolf/wolf_collar")
            };
            // The armor, then the cracks of its damage (`Crackiness.WOLF_ARMOR`).
            if !m.is_baby() && m.extra.body.as_deref() == Some("minecraft:wolf_armor") {
                let left = (super::super::WOLF_ARMOR_DURABILITY - m.extra.body_damage) as f32
                    / super::super::WOLF_ARMOR_DURABILITY as f32;
                push(
                    &mut v,
                    "wolf_armor#main",
                    "entity/equipment/wolf_body/armadillo_scute",
                    white,
                    false,
                );
                let crack = [(0.32, "high"), (0.69, "medium"), (0.95, "low")]
                    .iter()
                    .find(|(t, _)| left < *t);
                if let Some((_, level)) = crack {
                    push(
                        &mut v,
                        "wolf_armor#main",
                        interned(format_args!("entity/wolf/wolf_armor_crackiness_{level}")),
                        white,
                        false,
                    );
                }
            }
            push(
                &mut v,
                model,
                texture,
                abgr(dye(m.extra.color as usize)),
                false,
            );
        }
        // SimpleEquipmentLayer: the harness over the body.
        S::HappyGhast => {
            let harness = m
                .extra
                .body
                .as_deref()
                .and_then(super::super::happy_ghast::harness_texture);
            if !m.is_baby()
                && let Some(texture) = &harness
            {
                push(
                    &mut v,
                    "happy_ghast_harness#main",
                    texture.clone(),
                    white,
                    false,
                );
            }
            // RopesLayer: the ropes over a harnessed body while the ghast
            // holds a quad lead (`isLeashHolder`), the baby's model for a baby.
            if harness.is_some() && super::super::happy_ghast::is_leash_holder(m) {
                let model = if m.is_baby() {
                    "happy_ghast_baby_ropes#main"
                } else {
                    "happy_ghast_ropes#main"
                };
                push(
                    &mut v,
                    model,
                    "entity/ghast/happy_ghast_ropes",
                    white,
                    false,
                );
            }
        }
        // BreezeWindLayer and BreezeEyesLayer. The wind layer's texture
        // scrolls (`RenderTypes.breezeWind`): the offset is taken inside the
        // sprite, so the atlas does not have to wrap.
        S::Breeze => {
            v.push(Layer {
                model: "breeze#wind".into(),
                texture: "entity/breeze/breeze_wind".into(),
                color: white,
                full_bright: false,
                scroll: scroll_offset(age, BREEZE_WIND_SCROLL),
            });
            push(
                &mut v,
                "breeze#eyes",
                "entity/breeze/breeze_eyes",
                white,
                true,
            );
        }
        // LlamaDecorLayer: the carpet, or the trader's blanket.
        S::Llama | S::TraderLlama => {
            if let Some((model, texture)) = super::super::llama::decor(m) {
                push(&mut v, model, texture, white, false);
            }
        }
        // HorseMarkingLayer, the saddle and the horse armour.
        s if super::super::equine::is_equine(s) => {
            let name = super::super::equine::model_name(s);
            let body = if m.is_baby() {
                interned(format_args!("{name}_baby#main"))
            } else {
                interned(format_args!("{name}#main"))
            };
            if let Some(tex) = super::super::equine::markings(m) {
                push(&mut v, body, tex, white, false);
            }
            if m.extra.saddled && !m.is_baby() {
                push(
                    &mut v,
                    interned(format_args!("{name}#saddle")),
                    super::super::equine::saddle_texture(s),
                    white,
                    false,
                );
            }
            if let Some(armor) = m.extra.body.as_deref()
                && let Some(material) = armor
                    .strip_prefix("minecraft:")
                    .and_then(super::super::equine::horse_armor_asset)
            {
                let tex = interned(format_args!("entity/equipment/horse_body/{material}"));
                let color = if material == "leather" {
                    abgr(0xA06540)
                } else {
                    white
                };
                // UndeadHorseRenderer bakes its own armour layer.
                let layer = if matches!(s, S::ZombieHorse | S::SkeletonHorse) {
                    "undead_horse_armor#main"
                } else {
                    "horse_armor#main"
                };
                push(&mut v, layer, tex, color, false);
            }
        }
        // SimpleEquipmentLayer: pig and strider saddles.
        S::Pig if m.extra.saddled && !m.is_baby() => push(
            &mut v,
            "pig#saddle",
            "entity/equipment/pig_saddle/saddle",
            white,
            false,
        ),
        S::Strider if m.extra.saddled && !m.is_baby() => push(
            &mut v,
            "strider#saddle",
            "entity/equipment/strider_saddle/saddle",
            white,
            false,
        ),
        // CatCollarLayer.
        S::Cat if m.extra.tame => {
            let (model, texture) = if m.is_baby() {
                ("cat_baby#collar", "entity/cat/cat_collar_baby")
            } else {
                ("cat#collar", "entity/cat/cat_collar")
            };
            push(
                &mut v,
                model,
                texture,
                abgr(dye(m.extra.color as usize)),
                false,
            );
        }
        // IronGolemCrackinessLayer.
        S::IronGolem => {
            if let Some(texture) = super::super::golem::crackiness(m) {
                push(&mut v, "iron_golem#main", texture, white, false);
            }
        }
        // NautilusRenderer: the armour and saddle SimpleEquipmentLayers.
        S::Nautilus | S::ZombieNautilus if !m.is_baby() => {
            if let Some(texture) = m
                .extra
                .body
                .as_deref()
                .and_then(super::super::nautilus::armor_texture)
            {
                push(&mut v, "nautilus_armor#main", texture, white, false);
            }
            if m.extra.saddled {
                push(
                    &mut v,
                    "nautilus#saddle",
                    "entity/equipment/nautilus_saddle/saddle",
                    white,
                    false,
                );
            }
        }
        // SulfurCubeInnerLayer.
        S::SulfurCube => {
            if let Some((model, texture)) = super::super::sulfur_cube::inner(m) {
                push(&mut v, model, texture, white, false);
            }
        }
        // SlimeOuterLayer: the translucent outer cube.
        S::Slime => push(&mut v, "slime#outer", "entity/slime/slime", white, false),
        // SheepWoolUndercoatLayer has no isSheared guard: a sheared dyed sheep
        // keeps it, while only SheepWoolLayer stops.
        S::Sheep => {
            // MobRenderer.checkMagicName(entity, "jeb_"): an exact custom name.
            let jeb = m.custom_name.as_deref() == Some("jeb_");
            let color = abgr(if jeb {
                jeb_wool_rgb(age)
            } else {
                sheep_wool_rgb(m.extra.color)
            });
            if !m.is_baby() && (jeb || m.extra.color != 0) {
                push(
                    &mut v,
                    "sheep#wool_undercoat",
                    "entity/sheep/sheep_wool_undercoat",
                    color,
                    false,
                );
            }
            // SheepWoolLayer: none once sheared.
            if !m.extra.sheared {
                let (model, texture) = if m.is_baby() {
                    ("sheep_baby#wool", "entity/sheep/sheep_wool_baby")
                } else {
                    ("sheep#wool", "entity/sheep/sheep_wool")
                };
                push(&mut v, model, texture, color, false);
            }
        }
        S::Creaking if super::super::creaking::eyes_glowing(m) => push(
            &mut v,
            "creaking#eyes",
            "entity/creaking/creaking_eyes",
            white,
            true,
        ),
        // The copper golem's eyes glow: CopperGolemRenderer's
        // LivingEntityEmissiveLayer (eyes render type) draws on a model baked
        // from ModelLayers.COPPER_GOLEM, the body's, not COPPER_GOLEM_EYES.
        S::CopperGolem => {
            let eyes = super::super::copper_golem::eye_texture(m);
            push(&mut v, m.kind.model_name(), eyes, white, true);
        }
        S::Spider | S::CaveSpider => push(
            &mut v,
            m.kind.model_name(),
            "entity/spider/spider_eyes",
            white,
            true,
        ),
        S::Enderman => push(
            &mut v,
            "enderman#main",
            "entity/enderman/enderman_eyes",
            white,
            true,
        ),
        S::Phantom => push(
            &mut v,
            "phantom#main",
            "entity/phantom/phantom_eyes",
            white,
            true,
        ),
        S::Drowned => {
            if m.is_baby() {
                push(
                    &mut v,
                    "drowned_baby#outer",
                    "entity/zombie/drowned_outer_layer_baby",
                    white,
                    false,
                );
            } else {
                push(
                    &mut v,
                    "drowned#outer",
                    "entity/zombie/drowned_outer_layer",
                    white,
                    false,
                );
            }
        }
        S::Stray => push(
            &mut v,
            "stray#outer",
            "entity/skeleton/stray_overlay",
            white,
            false,
        ),
        S::Bogged if !m.extra.sheared => push(
            &mut v,
            "bogged#outer",
            "entity/skeleton/bogged_overlay",
            white,
            false,
        ),
        S::TropicalFish => {
            // TropicalFishPatternLayer: the pattern tinted with the pattern colour.
            let (model, letter, n) = if m.extra.variant < 6 {
                ("tropical_fish_small#pattern", "a", m.extra.variant + 1)
            } else {
                ("tropical_fish_large#pattern", "b", m.extra.variant - 5)
            };
            push(
                &mut v,
                model,
                interned(format_args!("entity/fish/tropical_{letter}_pattern_{n}")),
                abgr(dye(m.extra.variant3 as usize)),
                false,
            );
        }
        S::Villager | S::ZombieVillager => {
            let base = if m.species() == S::Villager {
                "villager"
            } else {
                "zombie_villager"
            };
            let model = if m.is_baby() {
                interned(format_args!("{base}_baby#main"))
            } else {
                interned(format_args!("{base}#main"))
            };
            let t = VILLAGER_TYPES[m.extra.variant as usize % VILLAGER_TYPES.len()];
            let folder = if m.species() == S::Villager {
                "entity/villager"
            } else {
                "entity/zombie_villager"
            };
            // VillagerProfessionLayer: a baby's type texture is in `baby/`.
            let group = if m.is_baby() { "baby" } else { "type" };
            push(
                &mut v,
                model.clone(),
                interned(format_args!("{folder}/{group}/{t}")),
                white,
                false,
            );
            let prof = PROFESSIONS[m.extra.variant2 as usize % PROFESSIONS.len()];
            if !m.is_baby() && prof != "none" {
                push(
                    &mut v,
                    model.clone(),
                    interned(format_args!("{folder}/profession/{prof}")),
                    white,
                    false,
                );
                if prof != "nitwit" {
                    // `LEVEL_LOCATIONS` clamps the level to 1..=5.
                    let level = ["stone", "iron", "gold", "emerald", "diamond"]
                        [(m.extra.variant3.max(1) as usize - 1).min(4)];
                    push(
                        &mut v,
                        model,
                        interned(format_args!("{folder}/profession_level/{level}")),
                        white,
                        false,
                    );
                }
            }
        }
        _ => {}
    }
    humanoid_armor(m, &mut v);
    v
}
/// `HumanoidArmorLayer.renderArmorPiece`: a baby wears the `humanoid_baby`
/// layer type for every slot (leggings included), an adult the leggings layer
/// for its legs and the `humanoid` one otherwise.
pub(super) fn humanoid_layer_folder(baby: bool, layer: &str) -> &'static str {
    if baby {
        "humanoid_baby"
    } else if layer == "leggings" {
        "humanoid_leggings"
    } else {
        "humanoid"
    }
}
/// `HumanoidArmorLayer`: the armour models of the mob's main model with the
/// equipment textures (leather tinted, with its overlay).
pub(super) fn humanoid_armor(m: &Mob, v: &mut Vec<Layer>) {
    // A player's or mannequin's armour is drawn by the game's armour layers.
    if super::super::equine::is_equine(m.species()) || avatar(m) {
        return;
    }
    let e = &m.extra;
    if e.head.is_none() && e.chest.is_none() && e.legs.is_none() && e.feet.is_none() {
        return;
    }
    let (model, _) = body(m);
    let Some(base) = model.strip_suffix("#main") else {
        return;
    };
    let slots = [
        (&m.extra.head, &m.extra.armor[0], "helmet"),
        (&m.extra.chest, &m.extra.armor[1], "chestplate"),
        (&m.extra.legs, &m.extra.armor[2], "leggings"),
        (&m.extra.feet, &m.extra.armor[3], "boots"),
    ];
    for (item, stack, layer) in slots {
        let Some(name) = item.as_deref().and_then(|i| i.strip_prefix("minecraft:")) else {
            continue;
        };
        let Some(material) = name.strip_suffix(layer).and_then(|n| n.strip_suffix('_')) else {
            continue;
        };
        let asset = match material {
            "golden" => "gold",
            "turtle" => "turtle_scute",
            other => other,
        };
        let folder = humanoid_layer_folder(m.is_baby(), layer);
        let model = interned(format_args!("{base}#{layer}"));
        // EquipmentLayerRenderer.getColorForLayer: the leather layer takes
        // the stack's dye colour, the colour it has when undyed otherwise.
        let tint = if asset == "leather" {
            armor_tint(Some(0xA06540), stack.as_ref())
        } else {
            0xFFFF_FFFF
        };
        v.push(Layer {
            model: model.clone(),
            texture: interned(format_args!("entity/equipment/{folder}/{asset}")),
            color: tint,
            full_bright: false,
            scroll: 0.0,
        });
        if asset == "leather" {
            v.push(Layer {
                model,
                texture: interned(format_args!("entity/equipment/{folder}/leather_overlay")),
                color: 0xFFFF_FFFF,
                full_bright: false,
                scroll: 0.0,
            });
        }
    }
}
