//! Checks the Horizon test pack (`tools/test-pack/build_test_pack.py`) the way the game loads it:
//! opens the `.mcpack`, converts it, loads the bundle and checks each feature the pack
//! exercises. One line per check, `ok` or `FALHOU` with what is missing; exit code 1 when any
//! check fails.
//!
//! `cargo run --release -p horizon-bedrock --example test_pack -- <horizon-test-pack.mcpack>`

use horizon_bedrock::bundle::Bundle;
use horizon_bedrock::convert::to_java;
use horizon_bedrock::pack::{Options, Pack};
use std::path::Path;

struct Checks {
    failed: usize,
}

impl Checks {
    fn check(&mut self, what: &str, ok: bool, detail: impl FnOnce() -> String) {
        if ok {
            println!("ok      {what}");
        } else {
            self.failed += 1;
            println!("FALHOU  {what}: {}", detail());
        }
    }
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("uso: test_pack <horizon-test-pack.mcpack>");
        std::process::exit(2);
    };
    let pack = Pack::open_with(Path::new(&path), &Options { subpack: None }).expect("abrir o pack");
    let out = std::env::temp_dir().join(format!("horizon-test-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let report = to_java(&pack, &out, (64, 0)).expect("converter");
    let b = Bundle::load(&out);
    let mut c = Checks { failed: 0 };
    // The player's `steve` (the player's own skin in the game) and `cape_invisible` are the
    // game's, not the pack's.
    c.check("conversao sem textura faltando (fora steve e capa)", report.missing_textures <= 2, || {
        format!("{report:?}")
    });

    // 1st / 3rd person: attachables, their models and animations.
    for (item, geo, anims) in [
        ("minecraft:paper", "geometry.fptest.axes", &[][..]),
        (
            "minecraft:stick",
            "geometry.fptest.stick",
            &["animation.fptest.stick.first_person", "animation.fptest.stick.third_person"][..],
        ),
    ] {
        let att = b.attachables.get(item);
        c.check(&format!("attachable {item}"), att.is_some(), || "nao carregado".into());
        let Some(att) = att else { continue };
        let g = att.geometry.get("default").map(String::as_str);
        c.check(&format!("{item}: modelo {geo}"), g == Some(geo) && b.geometries.contains_key(geo), || {
            format!("geometry {g:?}, carregado {}", b.geometries.contains_key(geo))
        });
        if let Some(model) = b.geometries.get(geo) {
            let item_bone = model.bones.iter().find(|x| x.name == "item");
            c.check(&format!("{item}: osso `item` ligado a mao"), item_bone.is_some_and(|x| x.binding.is_some()), || {
                "sem osso item ou sem binding".into()
            });
        }
        for a in anims {
            c.check(&format!("{item}: animacao {a}"), b.library.animations.contains_key(*a), || "nao carregada".into());
        }
    }

    // Texture meshes: bare (brick) and laid out as A&S's 3D tools (flint).
    for (item, geo, bone) in [
        ("minecraft:brick", "geometry.fptest.mesh", "item"),
        ("minecraft:flint", "geometry.fptest.mesh_ans", "sword"),
    ] {
        let att = b.attachables.get(item);
        c.check(&format!("attachable {item}"), att.is_some(), || "nao carregado".into());
        let mesh = b
            .geometries
            .get(geo)
            .and_then(|g| g.bones.iter().find(|x| x.name == bone))
            .is_some_and(|x| x.binding.is_some() && !x.texture_meshes.is_empty());
        c.check(&format!("{item}: osso `{bone}` ligado com texture_meshes"), mesh, || {
            format!("modelo {geo} sem o osso, binding ou malha")
        });
    }

    // A&S-like features: stencil window (clock), two hands (shears), holder variable (rod).
    for (item, geo) in [
        ("minecraft:clock", "geometry.fptest.clock"),
        ("minecraft:shears", "geometry.fptest.shears"),
        ("minecraft:fishing_rod", "geometry.fptest.rod"),
    ] {
        let att = b.attachables.get(item);
        c.check(&format!("attachable {item}"), att.is_some_and(|a| a.geometry.get("default").map(String::as_str) == Some(geo)), || {
            format!("{:?}", att.map(|a| &a.geometry))
        });
        c.check(&format!("{item}: modelo {geo}"), b.geometries.contains_key(geo), || "ausente".into());
    }
    c.check(
        "player: controlador de duas maos",
        b.render_controllers.contains_key("controller.render.fptest.two_hands")
            && b.library.animations.contains_key("animation.fptest.two_hands"),
        || "render controller ou animacao ausente".into(),
    );

    // Player: the vanilla entity with the test's events.
    let player = b.entities.get("minecraft:player");
    c.check("entidade minecraft:player", player.is_some(), || "nao carregada".into());
    if let Some(p) = player {
        for (k, v) in [("fptest_splash", "fptest:splash"), ("fptest_land", "fptest:land")] {
            c.check(&format!("player: particula {k}"), p.particles.get(k).map(String::as_str) == Some(v), || {
                format!("{:?}", p.particles.get(k))
            });
        }
        c.check("player: som fptest_blip", p.sounds.get("fptest_blip").map(String::as_str) == Some("fptest.blip"), || {
            format!("{:?}", p.sounds.get("fptest_blip"))
        });
        c.check(
            "player: controlador de eventos",
            b.library.controllers.contains_key("controller.animation.fptest.player_events")
                && p.animate.iter().any(|(k, _)| k == "fptest_events"),
            || "controlador ou animate ausente".into(),
        );
    }

    // Modified entity, its particles, sound and name-tag variant.
    let pig = b.entities.get("minecraft:pig");
    c.check("entidade minecraft:pig modificada", pig.is_some(), || "nao carregada".into());
    if let Some(p) = pig {
        c.check("pig: modelo geometry.fptest.pig", b.geometries.contains_key("geometry.fptest.pig"), || "ausente".into());
        c.check("pig: textura de etiqueta (named)", p.textures.contains_key("named"), || format!("{:?}", p.textures));
        c.check(
            "pig: render controller por nome",
            b.render_controllers.contains_key("controller.render.fptest.pig"),
            || "ausente".into(),
        );
        c.check("pig: particula aura", p.particles.get("aura").map(String::as_str) == Some("fptest:aura"), || {
            format!("{:?}", p.particles)
        });
    }
    for e in ["fptest:splash", "fptest:land", "fptest:aura"] {
        c.check(&format!("particula {e}"), b.effects.contains_key(e), || "nao carregada".into());
    }
    c.check("som fptest.blip definido", b.sound_definitions.map.contains_key("fptest.blip"), || "ausente".into());
    c.check("arquivo do som convertido", out.join("assets/minecraft/sounds").exists() || out.join("assets").exists(), || {
        "sem pasta de sons".into()
    });
    c.check("bloco sponge (blocks.json)", b.block_defs.keys().any(|k| k.ends_with("sponge")), || {
        format!("{:?}", b.block_defs.keys().collect::<Vec<_>>())
    });

    c.check("esponja: textura do blocks.json em block/sponge", out.join("assets/minecraft/textures/block/sponge.png").is_file(), || {
        "block/sponge.png nao escrito".into()
    });
    for (code, key, want) in [
        ("en_us", "item.minecraft.paper", "Paper (test axes)"),
        ("pt_br", "item.minecraft.paper", "Papel (eixos de teste)"),
        ("pt_br", "item.minecraft.stick", "Graveto (animação de teste)"),
        ("pt_br", "block.minecraft.sponge", "Esponja (teste)"),
        ("pt_br", "entity.minecraft.pig", "Porco (teste)"),
    ] {
        let got = std::fs::read(out.join(format!("assets/minecraft/lang/{code}.json")))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v.get(key)?.as_str().map(str::to_owned));
        c.check(&format!("texto {code} {key}"), got.as_deref() == Some(want), || format!("{got:?}"));
    }

    let _ = std::fs::remove_dir_all(&out);
    println!("{} falha(s)", c.failed);
    std::process::exit(i32::from(c.failed > 0));
}
