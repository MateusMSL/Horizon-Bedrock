# Partículas do pack no Horizon

Gerado por `tools/bedrock/particles_checklist.py`: 525 efeitos, 45 substituem partículas vanilla do Bedrock, 480 do próprio pack.

## Substitutas de partículas vanilla

O Horizon entrega ao efeito do pack a partícula Java da coluna *Java* (`bedrock_particles.rs`), quando o efeito faz uma partícula por pedido (`manual` ou rajada pequena).

| ok | efeito | Java | taxa | arquivo |
| --- | --- | --- | --- | --- |
| [ ] | `minecraft:arrow_spell_emitter` | sem equivalente | instant | eex.json |
| [ ] | `minecraft:basic_crit_particle` | `Crit` | instant | eev.json |
| [ ] | `minecraft:block_destruct` | sem equivalente | instant | ept.json |
| [ ] | `minecraft:campfire_smoke_particle` | `CampfireCosySmoke` | manual | eqb.json |
| [ ] | `minecraft:campfire_tall_smoke_particle` | `CampfireSignalSmoke` | manual | efe.json |
| [ ] | `minecraft:candle_flame_particle` | `SmallFlame` | manual | nin.json |
| [ ] | `minecraft:critical_hit_emitter` | sem equivalente | instant | efm.json |
| [ ] | `minecraft:death_explosion_emitter` | sem equivalente | instant | rmt.json |
| [ ] | `minecraft:dragon_breath_fire` | sem equivalente | instant | epw.json |
| [ ] | `minecraft:dragon_breath_lingering` | sem equivalente | instant | efh.json |
| [ ] | `minecraft:dragon_breath_trail` | `DragonBreath` | manual | epy.json |
| [ ] | `minecraft:dragon_death_explosion_emitter` | sem equivalente | instant | mgs.json |
| [ ] | `minecraft:dragon_dying_explosion` | sem equivalente | instant | rmu.json |
| [ ] | `minecraft:enchanting_table_particle` | `Enchant` | instant | epv.json |
| [ ] | `minecraft:end_chest` | sem equivalente | instant | efd.json |
| [ ] | `minecraft:firefly_particle` | `Firefly` | instant | efi.json |
| [ ] | `minecraft:geyser_base` | `GeyserBase` não: o efeito é um fluxo, a partícula do jogo fica | steady | nxd.json |
| [ ] | `minecraft:glow_particle` | `Glow` | instant | mgr.json |
| [ ] | `minecraft:heart_particle` | `Heart` | manual | epx.json |
| [ ] | `minecraft:knockback_roar_particle` | sem equivalente | instant | eqa.json |
| [ ] | `minecraft:mob_portal` | `Portal` | manual | efg.json |
| [ ] | `minecraft:mobflame_emitter` | sem equivalente | steady | efn.json |
| [ ] | `minecraft:mobspell_emitter` | sem equivalente | instant | eez.json |
| [ ] | `minecraft:noxious_gas_particle` | `NoxiousGas` não: o efeito é um fluxo, a partícula do jogo fica | steady | nxa.json |
| [ ] | `minecraft:orange_poplar_leaves_particle` | `OrangePoplarLeaves` | manual | pry.json |
| [ ] | `minecraft:pause_mob_growth` | `PauseMobGrowth` | instant | mtm.json |
| [ ] | `minecraft:phantom_trail` | sem equivalente | steady | eey.json |
| [ ] | `minecraft:portal_directional` | sem equivalente | instant | eqc.json |
| [ ] | `minecraft:portal_east_west` | sem equivalente | instant | efc.json |
| [ ] | `minecraft:portal_north_south` | sem equivalente | instant | eps.json |
| [ ] | `minecraft:portal_reverse_particle` | `ReversePortal` não: o efeito é um fluxo, a partícula do jogo fica | steady | efa.json |
| [ ] | `minecraft:rain_splash_particle` | `Rain` | manual | mgq.json |
| [ ] | `minecraft:red_poplar_leaves_particle` | `RedPoplarLeaves` | manual | prz.json |
| [ ] | `minecraft:reset_mob_growth` | `ResetMobGrowth` | instant | mtl.json |
| [ ] | `minecraft:shulker_bullet` | sem equivalente | manual | epz.json |
| [ ] | `minecraft:splash_spell_emitter` | sem equivalente | instant | efo.json |
| [ ] | `minecraft:sulfur_bubble_particle` | `SulfurBubbles` não: o efeito é um fluxo, a partícula do jogo fica | steady | nxc.json |
| [ ] | `minecraft:sulfur_cube_goo` | `SulfurCubeGoo` | instant | nxb.json |
| [ ] | `minecraft:totem_manual` | `TotemOfUndying` | manual | efl.json |
| [ ] | `minecraft:totem_particle` | `TotemOfUndying` não: o efeito é um fluxo, a partícula do jogo fica | steady | eew.json |
| [ ] | `minecraft:villager_happy` | `HappyVillager` | manual | efb.json |
| [ ] | `minecraft:warden_dig` | sem equivalente | instant | epu.json |
| [ ] | `minecraft:water_splash_particle_manual` | `Splash` | manual | eff.json |
| [ ] | `minecraft:water_wake_particle` | `Fishing` | instant | pqq.json |
| [ ] | `minecraft:yellow_poplar_leaves_particle` | `YellowPoplarLeaves` | manual | psa.json |

## Efeitos do pack

Disparados por animações/controladores das entidades e attachables que os nomeiam.

| ok | efeito | taxa | usado por |
| --- | --- | --- | --- |
| [ ] | `oreville_ans:adyvkf` | instant | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:aegcwd` | instant | partícula `res.json` |
| [ ] | `oreville_ans:aejbkk` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:aigfhq` | instant | entidade `minecraft:iron_golem`, entidade `minecraft:warden` |
| [ ] | `oreville_ans:ajivwq` | steady | attachable `minecraft:powder_snow_bucket` |
| [ ] | `oreville_ans:akbkls` | instant | entidade `minecraft:piglin`, entidade `minecraft:villager_v2`, entidade `minecraft:wandering_trader` |
| [ ] | `oreville_ans:akhtql` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:alcsre` | instant | partícula `rga.json` |
| [ ] | `oreville_ans:annoxl` | instant | partícula `rgh.json` |
| [ ] | `oreville_ans:apqgad` | steady | - |
| [ ] | `oreville_ans:aqspda` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:arotgo` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:audgzy` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:aupsst` | instant | - |
| [ ] | `oreville_ans:awewku` | instant | entidade `minecraft:elder_guardian`, entidade `minecraft:sniffer` |
| [ ] | `oreville_ans:awoviv` | steady | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:aynkct` | steady | attachable `minecraft:arrow`, entidade `minecraft:arrow` |
| [ ] | `oreville_ans:banufe` | instant | attachable `minecraft:chest_minecart`, attachable `minecraft:command_block_minecart`, attachable `minecraft:hopper_minecart`, attachable `minecraft:minecart` e mais 6 |
| [ ] | `oreville_ans:bcdjqk` | steady | entidade `minecraft:wither` |
| [ ] | `oreville_ans:beetroot_seeds` | instant | attachable `minecraft:beetroot_seeds` |
| [ ] | `oreville_ans:bgpbtm` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:bguunf` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:bhpxmq` | instant | partícula `rhi.json` |
| [ ] | `oreville_ans:bntjjk` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:bobiol` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:bobtew` | steady | entidade `minecraft:wind_charge_projectile` |
| [ ] | `oreville_ans:boegsz` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:bpbjus` | steady | entidade `minecraft:wandering_trader`, entidade `minecraft:witch` |
| [ ] | `oreville_ans:bpyggn` | steady | entidade `minecraft:nautilus`, entidade `minecraft:zombie_nautilus` |
| [ ] | `oreville_ans:bqrhun` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:brvpco` | instant | partícula `rej.json` |
| [ ] | `oreville_ans:bscxeh` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:bsmhvx` | instant | entidade `minecraft:elder_guardian`, entidade `minecraft:polar_bear`, entidade `minecraft:ravager`, entidade `minecraft:sniffer` |
| [ ] | `oreville_ans:bvivod` | instant | attachable `minecraft:amethyst_cluster`, attachable `minecraft:amethyst_shard`, attachable `minecraft:diamond`, attachable `minecraft:emerald` e mais 8 |
| [ ] | `oreville_ans:bzwpoo` | instant | partícula `rgj.json` |
| [ ] | `oreville_ans:cciaww` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:cestwf` | instant | attachable `minecraft:lit_pumpkin` |
| [ ] | `oreville_ans:cfwekf` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:cioyls` | instant | partícula `egh.json`, partícula `ehr.json`, partícula `eig.json`, partícula `ejd.json` e mais 4 |
| [ ] | `oreville_ans:ckxyli` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:cmrola` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:cohrbk` | instant | entidade `minecraft:wither` |
| [ ] | `oreville_ans:cohrrz` | instant | partícula `res.json` |
| [ ] | `oreville_ans:corpjb` | instant | attachable `minecraft:soul_lantern` |
| [ ] | `oreville_ans:cotanr` | instant | entidade `minecraft:guardian` |
| [ ] | `oreville_ans:cppupb` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:cqkunk` | instant | partícula `ekq.json` |
| [ ] | `oreville_ans:cqwgvk` | instant | entidade `minecraft:evocation_fang` |
| [ ] | `oreville_ans:cqznrp` | instant | partícula `rej.json` |
| [ ] | `oreville_ans:csjeqn` | instant | partícula `rej.json` |
| [ ] | `oreville_ans:culanl` | steady | attachable `minecraft:furnace` |
| [ ] | `oreville_ans:cvocbd` | steady | entidade `minecraft:breeze` |
| [ ] | `oreville_ans:cwnppz` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:cwuosc` | steady | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:cxvcuv` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:dbbeqz` | instant | partícula `res.json` |
| [ ] | `oreville_ans:dcbgep` | steady | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:dcvxvq` | instant | partícula `rfg.json`, partícula `rfu.json` |
| [ ] | `oreville_ans:dflvus` | instant | partícula `rgn.json` |
| [ ] | `oreville_ans:dfqtjy` | instant | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:dmbcam` | manual | partícula `nqe.json` |
| [ ] | `oreville_ans:doxwba` | steady | entidade `minecraft:xp_orb` |
| [ ] | `oreville_ans:dpixec` | instant | entidade `minecraft:iron_golem` |
| [ ] | `oreville_ans:dragon_breath` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:drnrvi` | steady | attachable `minecraft:brick_block` |
| [ ] | `oreville_ans:dsdrzi` | instant | attachable `minecraft:chiseled_stone_bricks` |
| [ ] | `oreville_ans:dueruf_spawn_egg` | instant | attachable `minecraft:enderman_spawn_egg`, attachable `minecraft:endermite_spawn_egg`, attachable `minecraft:spawn_egg` |
| [ ] | `oreville_ans:dvrncm` | instant | partícula `pqc.json` |
| [ ] | `oreville_ans:dybimo` | steady | entidade `minecraft:bogged`, entidade `minecraft:enderman`, entidade `minecraft:evocation_illager`, entidade `minecraft:parched` e mais 6 |
| [ ] | `oreville_ans:dymxlh` | instant | partícula `eiy.json` |
| [ ] | `oreville_ans:dzzsci` | steady | entidade `minecraft:copper_golem`, entidade `minecraft:creeper` |
| [ ] | `oreville_ans:ecwhcg` | instant | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:edlemj` | steady | entidade `minecraft:drowned`, entidade `minecraft:husk`, entidade `minecraft:piglin_brute`, entidade `minecraft:piglin` e mais 3 |
| [ ] | `oreville_ans:efisym` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:egbttn` | instant | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:eheose` | instant | entidade `minecraft:dragon_fireball` |
| [ ] | `oreville_ans:ejwqin` | instant | partícula `ejl.json` |
| [ ] | `oreville_ans:eksuev` | instant | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:ektiop` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:embovt` | instant | entidade `minecraft:zombie` |
| [ ] | `oreville_ans:end_crystal` | instant | entidade `minecraft:ender_crystal` |
| [ ] | `oreville_ans:endckw` | instant | - |
| [ ] | `oreville_ans:epetyr` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:epwrzb` | instant | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:erktxq` | instant | partícula `rgp.json`, partícula `rgr.json`, partícula `rgs.json`, partícula `riv.json` e mais 2 |
| [ ] | `oreville_ans:etopxu` | instant | attachable `minecraft:chicken`, attachable `minecraft:rabbit`, entidade `minecraft:armadillo`, entidade `minecraft:axolotl` e mais 15 |
| [ ] | `oreville_ans:etphax` | instant | entidade `minecraft:snow_golem` |
| [ ] | `oreville_ans:evcxga` | instant | partícula `ekq.json` |
| [ ] | `oreville_ans:ewxmuz` | instant | partícula `nqj.json` |
| [ ] | `oreville_ans:ezietv` | instant | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:ezldmk` | instant | entidade `minecraft:camel_husk`, entidade `minecraft:camel`, entidade `minecraft:cave_spider`, entidade `minecraft:cow` e mais 8 |
| [ ] | `oreville_ans:fbnpyn` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:fdxqqv` | instant | partícula `ept.json` |
| [ ] | `oreville_ans:fgaqsv` | instant | attachable `minecraft:leaf_litter` |
| [ ] | `oreville_ans:fivuiy` | instant | attachable `minecraft:warped_fungus_on_a_stick` |
| [ ] | `oreville_ans:fllzkp` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:fmpdzx` | instant | attachable `minecraft:chest_minecart`, attachable `minecraft:command_block_minecart`, attachable `minecraft:hopper_minecart`, attachable `minecraft:minecart` e mais 24 |
| [ ] | `oreville_ans:fmxwfb` | instant | partícula `res.json` |
| [ ] | `oreville_ans:fqjgoq` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:fqsruh` | instant | partícula `rej.json` |
| [ ] | `oreville_ans:frame` | instant | entidade `minecraft:vex` |
| [ ] | `oreville_ans:fvhgxn` | instant | entidade `minecraft:frog` |
| [ ] | `oreville_ans:fwpjib` | instant | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:fztxyd` | instant | partícula `rgn.json` |
| [ ] | `oreville_ans:gcpawo` | steady | entidade `minecraft:wither` |
| [ ] | `oreville_ans:gctgse` | steady | attachable `minecraft:torch` |
| [ ] | `oreville_ans:gdlccj` | instant | entidade `minecraft:elder_guardian` |
| [ ] | `oreville_ans:gfdfpf` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:gkebgl` | instant | partícula `rii.json`, partícula `rik.json`, partícula `ril.json`, partícula `riu.json` |
| [ ] | `oreville_ans:gkmhpy` | steady | partícula `rid.json` |
| [ ] | `oreville_ans:grtjxg` | instant | - |
| [ ] | `oreville_ans:gsqiuw` | instant | entidade `minecraft:evocation_illager`, entidade `minecraft:piglin_brute`, entidade `minecraft:piglin`, entidade `minecraft:pillager` e mais 2 |
| [ ] | `oreville_ans:gsxagh` | steady | entidade `minecraft:fireball` |
| [ ] | `oreville_ans:gudhqp` | instant | entidade `minecraft:vex` |
| [ ] | `oreville_ans:gvzgfe` | steady | attachable `minecraft:bow` |
| [ ] | `oreville_ans:gywhhe` | instant | attachable `minecraft:carrot_on_a_stick`, attachable `oreville_ans:fishing_rod`, attachable `oreville_ans:vyiidn` |
| [ ] | `oreville_ans:hblgqy` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:hdwkpf` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:heorkq` | steady | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:hhxlpv` | instant | partícula `reg.json`, partícula `reh.json` |
| [ ] | `oreville_ans:hjekhu` | steady | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:hkdtif` | instant | partícula `nqc.json` |
| [ ] | `oreville_ans:hlqjil` | instant | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:hmkxlr` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:hmmhkh` | steady | attachable `minecraft:torch` |
| [ ] | `oreville_ans:hnsoxk` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:hqhjri` | instant | attachable `minecraft:cake` |
| [ ] | `oreville_ans:hrgoll` | instant | entidade `minecraft:wither` |
| [ ] | `oreville_ans:hrhrye` | instant | attachable `minecraft:flint_and_steel` |
| [ ] | `oreville_ans:hrihlh` | steady | entidade `minecraft:fireworks_rocket` |
| [ ] | `oreville_ans:hrkndd` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:hsfnkm` | instant | attachable `minecraft:furnace` |
| [ ] | `oreville_ans:htdwpk` | instant | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:htyzwr` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:huruot` | steady | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:huxwmr` | instant | partícula `mgp.json` |
| [ ] | `oreville_ans:hvbtrt` | instant | attachable `minecraft:wheat` |
| [ ] | `oreville_ans:hwbujj` | instant | attachable `minecraft:skeleton_skull` |
| [ ] | `oreville_ans:hzxbiw` | instant | partícula `ren.json`, partícula `rev.json`, partícula `rgo.json`, partícula `rgt.json` e mais 4 |
| [ ] | `oreville_ans:iaxlmq` | instant | partícula `res.json` |
| [ ] | `oreville_ans:ignwpa` | instant | partícula `rek.json`, partícula `rgf.json`, partícula `rgg.json`, partícula `rig.json` |
| [ ] | `oreville_ans:ijbfqy` | steady | - |
| [ ] | `oreville_ans:ijrkyb` | instant | attachable `minecraft:pufferfish`, attachable `minecraft:rabbit`, entidade `minecraft:armadillo`, entidade `minecraft:axolotl` e mais 8 |
| [ ] | `oreville_ans:imgysm` | instant | partícula `rii.json`, partícula `rik.json`, partícula `ril.json`, partícula `riu.json` |
| [ ] | `oreville_ans:imsyfq` | instant | entidade `minecraft:allay` |
| [ ] | `oreville_ans:ionbtf` | instant | partícula `rel.json`, partícula `rhi.json` |
| [ ] | `oreville_ans:iqcijl` | instant | attachable `minecraft:crossbow` |
| [ ] | `oreville_ans:iqfdev` | instant | partícula `rgp.json`, partícula `rgr.json`, partícula `rgs.json` |
| [ ] | `oreville_ans:isllnu` | steady | entidade `minecraft:wither` |
| [ ] | `oreville_ans:iveucv` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:iydhry` | steady | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:izbvph` | instant | partícula `eka.json`, partícula `eok.json` |
| [ ] | `oreville_ans:izrbmt` | steady | entidade `minecraft:phantom` |
| [ ] | `oreville_ans:jbemfh` | instant | partícula `rgg.json`, partícula `rgs.json`, partícula `ril.json`, partícula `riz.json` |
| [ ] | `oreville_ans:jceddx` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:jexawc` | manual | partícula `eky.json`, partícula `eow.json`, partícula `nqr.json`, partícula `pqa.json` |
| [ ] | `oreville_ans:jojqkm` | instant | entidade `minecraft:blaze` |
| [ ] | `oreville_ans:jokpgk` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:jralxs` | manual | partícula `nqo.json` |
| [ ] | `oreville_ans:jrjcmz` | instant | entidade `minecraft:phantom` |
| [ ] | `oreville_ans:jwttvk` | instant | partícula `mtl.json` |
| [ ] | `oreville_ans:jxbvwo` | instant | attachable `minecraft:red_candle` |
| [ ] | `oreville_ans:jxmvgv` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:jywftx` | instant | entidade `minecraft:cave_spider`, entidade `minecraft:copper_golem`, entidade `minecraft:guardian`, entidade `minecraft:spider` |
| [ ] | `oreville_ans:kbchwv` | instant | partícula `rjg.json` |
| [ ] | `oreville_ans:kbjmol` | instant | attachable `minecraft:brown_mushroom` |
| [ ] | `oreville_ans:kcwgra` | steady | - |
| [ ] | `oreville_ans:kdapmp` | instant | partícula `emd.json` |
| [ ] | `oreville_ans:kdcmgl` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:kdhnlq` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:kdoftm` | steady | attachable `minecraft:milk_bucket` |
| [ ] | `oreville_ans:kjnksh` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:kjwafo` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:kkjkgg` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:kldvfp` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:klifpz` | instant | entidade `minecraft:fishing_hook` |
| [ ] | `oreville_ans:kllrig` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:kuewhd` | instant | partícula `rga.json` |
| [ ] | `oreville_ans:kwnhvt` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:kzravn` | instant | partícula `rgj.json`, partícula `rgp.json`, partícula `rgr.json` |
| [ ] | `oreville_ans:lbpjyu` | instant | partícula `rgb.json` |
| [ ] | `oreville_ans:leaf_litter` | instant | attachable `minecraft:leaf_litter` |
| [ ] | `oreville_ans:lgeanj` | instant | partícula `rhn.json` |
| [ ] | `oreville_ans:lhcxiq` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:lhoudj` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:liqwoj` | instant | partícula `rek.json`, partícula `rgf.json`, partícula `rgg.json`, partícula `rig.json` |
| [ ] | `oreville_ans:ljcprd` | instant | attachable `minecraft:chorus_fruit` |
| [ ] | `oreville_ans:ljcqzr` | steady | entidade `minecraft:cave_spider`, entidade `minecraft:copper_golem`, entidade `minecraft:guardian`, entidade `minecraft:spider` |
| [ ] | `oreville_ans:lokpmz` | steady | entidade `minecraft:cat`, entidade `minecraft:fox` |
| [ ] | `oreville_ans:lqikkd` | instant | partícula `rig.json`, partícula `riu.json` |
| [ ] | `oreville_ans:lqmccn` | steady | partícula `rhn.json` |
| [ ] | `oreville_ans:lqmyue` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:lsnldk` | steady | - |
| [ ] | `oreville_ans:lvaefr` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:lvmvxu` | steady | attachable `minecraft:soul_torch` |
| [ ] | `oreville_ans:lwjydg` | instant | entidade `minecraft:sniffer` |
| [ ] | `oreville_ans:lwlfdf` | steady | entidade `minecraft:fishing_hook` |
| [ ] | `oreville_ans:lwvwsc` | instant | partícula `rgp.json`, partícula `rgr.json`, partícula `rgs.json`, partícula `rgz.json` |
| [ ] | `oreville_ans:maczxf` | steady | entidade `minecraft:breeze` |
| [ ] | `oreville_ans:maikvy` | steady | entidade `minecraft:dragon_fireball` |
| [ ] | `oreville_ans:mbhzsr` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:mbigdu` | instant | entidade `minecraft:wither_skeleton` |
| [ ] | `oreville_ans:mdaveq` | steady | attachable `minecraft:redstone_torch` |
| [ ] | `oreville_ans:mdeyzy` | instant | partícula `rga.json` |
| [ ] | `oreville_ans:medmjg` | instant | partícula `rik.json` |
| [ ] | `oreville_ans:melon_seeds` | instant | attachable `minecraft:melon_seeds` |
| [ ] | `oreville_ans:metamq` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:mfpmxb` | instant | partícula `rhi.json` |
| [ ] | `oreville_ans:mfvnui` | instant | attachable `minecraft:brown_mushroom` |
| [ ] | `oreville_ans:mguxav` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:mhjboa` | instant | partícula `rgb.json` |
| [ ] | `oreville_ans:mhrilz` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:mlozwz` | instant | entidade `minecraft:wither_skull_dangerous`, entidade `minecraft:wither_skull` |
| [ ] | `oreville_ans:mmlzgg` | instant | partícula `rhx.json`, partícula `ria.json` |
| [ ] | `oreville_ans:mngdyk` | instant | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:mtlnnp` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:musfoh` | instant | attachable `minecraft:lava_bucket` |
| [ ] | `oreville_ans:mvjojt` | instant | entidade `minecraft:spider` |
| [ ] | `oreville_ans:mwbbqh` | steady | partícula `mtl.json` |
| [ ] | `oreville_ans:mwiakz` | instant | partícula `rhd.json` |
| [ ] | `oreville_ans:mxlnse` | steady | attachable `minecraft:powder_snow_bucket` |
| [ ] | `oreville_ans:myqcko` | manual | partícula `nqi.json` |
| [ ] | `oreville_ans:mzawnd` | steady | entidade `minecraft:wandering_trader` |
| [ ] | `oreville_ans:mzhrgr` | steady | partícula `rii.json`, partícula `rik.json`, partícula `ril.json`, partícula `riu.json` |
| [ ] | `oreville_ans:mzocys` | instant | entidade `minecraft:allay`, entidade `minecraft:player`, entidade `minecraft:shulker`, entidade `minecraft:vex` |
| [ ] | `oreville_ans:mzpjxu` | instant | partícula `rel.json` |
| [ ] | `oreville_ans:naoyov` | steady | partícula `nxd.json` |
| [ ] | `oreville_ans:nbhgcd` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:nbrqwp` | instant | attachable `minecraft:arrow`, attachable `minecraft:cod`, attachable `minecraft:egg`, attachable `minecraft:ender_pearl` e mais 15 |
| [ ] | `oreville_ans:ndvctz` | instant | attachable `minecraft:chest_minecart`, attachable `minecraft:command_block_minecart`, attachable `minecraft:hopper_minecart`, attachable `minecraft:minecart` e mais 24 |
| [ ] | `oreville_ans:nhewct` | instant | entidade `minecraft:camel_husk`, entidade `minecraft:camel`, entidade `minecraft:elder_guardian`, entidade `minecraft:polar_bear` e mais 2 |
| [ ] | `oreville_ans:nifpmx` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:nijswo` | instant | partícula `pqq.json` |
| [ ] | `oreville_ans:nilpnj` | instant | entidade `minecraft:iron_golem` |
| [ ] | `oreville_ans:nkwaka` | instant | partícula `mtm.json` |
| [ ] | `oreville_ans:nnedkx` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:nnpyjs` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:noidvc` | instant | entidade `minecraft:evocation_fang` |
| [ ] | `oreville_ans:npavuy` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:npkvck` | steady | attachable `minecraft:chicken`, attachable `minecraft:pufferfish`, attachable `minecraft:rabbit`, entidade `minecraft:axolotl` e mais 8 |
| [ ] | `oreville_ans:npmwuq` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:npskzd` | instant | partícula `rgb.json` |
| [ ] | `oreville_ans:nszmav` | instant | entidade `minecraft:evocation_illager`, entidade `minecraft:piglin_brute`, entidade `minecraft:piglin`, entidade `minecraft:pillager` e mais 2 |
| [ ] | `oreville_ans:ntaytt` | instant | attachable `minecraft:lava_bucket` |
| [ ] | `oreville_ans:ntxisx` | instant | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:nuonev` | steady | attachable `minecraft:campfire`, attachable `minecraft:soul_campfire` |
| [ ] | `oreville_ans:nwgqsx` | steady | attachable `minecraft:potent_sulfur` |
| [ ] | `oreville_ans:nzjuzl` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:nzwsxs` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:obfchp` | steady | entidade `minecraft:allay` |
| [ ] | `oreville_ans:ocbxzl` | instant | partícula `rhd.json` |
| [ ] | `oreville_ans:occdmm` | instant | entidade `minecraft:fireball` |
| [ ] | `oreville_ans:ocfbzk` | steady | entidade `minecraft:copper_golem`, entidade `minecraft:creeper` |
| [ ] | `oreville_ans:oeogwf` | steady | attachable `minecraft:snowball`, entidade `minecraft:snowball` |
| [ ] | `oreville_ans:ogjuya` | steady | entidade `minecraft:elder_guardian`, entidade `minecraft:polar_bear`, entidade `minecraft:ravager`, entidade `minecraft:sniffer` |
| [ ] | `oreville_ans:okntly` | instant | partícula `efx.json` |
| [ ] | `oreville_ans:ondhvu` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:opsoho` | instant | entidade `minecraft:turtle` |
| [ ] | `oreville_ans:otgche` | instant | entidade `minecraft:warden` |
| [ ] | `oreville_ans:oulepo` | manual | partícula `nqw.json` |
| [ ] | `oreville_ans:ovfqxf` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:oxtnje` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:ozlclk` | instant | entidade `minecraft:husk` |
| [ ] | `oreville_ans:paljdo` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:pauuah` | instant | partícula `rhi.json` |
| [ ] | `oreville_ans:pbhfex` | instant | partícula `ekf.json` |
| [ ] | `oreville_ans:pbrinp` | instant | partícula `nqt.json` |
| [ ] | `oreville_ans:pdabuv` | instant | entidade `minecraft:bogged`, entidade `minecraft:cave_spider`, entidade `minecraft:copper_golem`, entidade `minecraft:creaking` e mais 30 |
| [ ] | `oreville_ans:pexwmm` | steady | partícula `rfi.json` |
| [ ] | `oreville_ans:pink_petals` | instant | attachable `minecraft:pink_petals` |
| [ ] | `oreville_ans:pkuujn` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:pllwyf` | manual | partícula `nxc.json` |
| [ ] | `oreville_ans:pmjsej` | instant | entidade `minecraft:donkey`, entidade `minecraft:horse`, entidade `minecraft:mule`, entidade `minecraft:skeleton_horse` e mais 1 |
| [ ] | `oreville_ans:pnmias` | instant | entidade `minecraft:iron_golem` |
| [ ] | `oreville_ans:pomxba` | instant | partícula `rgz.json` |
| [ ] | `oreville_ans:poudpm` | steady | entidade `minecraft:copper_golem` |
| [ ] | `oreville_ans:ppmmod` | instant | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:ppwnbw` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:prseyf` | instant | entidade `minecraft:breeze` |
| [ ] | `oreville_ans:ptsoae` | instant | partícula `rhq.json` |
| [ ] | `oreville_ans:puilgq` | steady | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:pumpkin_seeds` | instant | attachable `minecraft:pumpkin_seeds` |
| [ ] | `oreville_ans:puualg` | steady | partícula `rfg.json` |
| [ ] | `oreville_ans:pvubiw` | instant | partícula `mgp.json`, partícula `pqk.json`, partícula `pqn.json` |
| [ ] | `oreville_ans:pwiugf` | instant | partícula `rfv.json` |
| [ ] | `oreville_ans:pxabgv` | instant | entidade `minecraft:warden` |
| [ ] | `oreville_ans:pxnpjx` | instant | partícula `eon.json` |
| [ ] | `oreville_ans:pyrlzn` | instant | partícula `rgj.json` |
| [ ] | `oreville_ans:qaeghi` | instant | partícula `eoy.json` |
| [ ] | `oreville_ans:qbkcbp` | steady | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:qbqawe` | instant | partícula `pqe.json` |
| [ ] | `oreville_ans:qccqxw` | instant | entidade `minecraft:fishing_hook` |
| [ ] | `oreville_ans:qeracd` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:qgbipi` | steady | entidade `minecraft:armadillo`, entidade `minecraft:cat`, entidade `minecraft:dolphin`, entidade `minecraft:fox` e mais 5 |
| [ ] | `oreville_ans:qgfxus` | instant | partícula `riv.json`, partícula `riy.json`, partícula `riz.json`, partícula `rjg.json` |
| [ ] | `oreville_ans:qhbvmn` | instant | partícula `mgq.json` |
| [ ] | `oreville_ans:qhhhej` | instant | attachable `minecraft:armor_stand`, attachable `minecraft:arrow`, attachable `minecraft:bed`, attachable `minecraft:blue_egg` e mais 115 |
| [ ] | `oreville_ans:qinwcg` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:qkaypc` | instant | partícula `rgp.json`, partícula `rgr.json`, partícula `rgs.json`, partícula `rgz.json` |
| [ ] | `oreville_ans:qknxqt` | instant | partícula `rel.json` |
| [ ] | `oreville_ans:qlegdk` | steady | attachable `minecraft:enchanted_golden_apple`, attachable `minecraft:golden_apple` |
| [ ] | `oreville_ans:qmmsvr` | steady | attachable `minecraft:milk_bucket` |
| [ ] | `oreville_ans:qokzhq` | instant | entidade `minecraft:xp_orb` |
| [ ] | `oreville_ans:qoufgh` | instant | partícula `ego.json` |
| [ ] | `oreville_ans:qouoou` | instant | entidade `minecraft:bogged`, entidade `minecraft:cave_spider`, entidade `minecraft:copper_golem`, entidade `minecraft:creaking` e mais 30 |
| [ ] | `oreville_ans:qpgdll` | instant | entidade `minecraft:witch` |
| [ ] | `oreville_ans:qqouwr` | steady | attachable `minecraft:torch` |
| [ ] | `oreville_ans:qqpiff` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:qqtfzr` | instant | partícula `efp.json` |
| [ ] | `oreville_ans:qrbcig` | instant | entidade `minecraft:donkey`, entidade `minecraft:horse`, entidade `minecraft:mule`, entidade `minecraft:skeleton_horse` e mais 1 |
| [ ] | `oreville_ans:qsfsjo` | instant | partícula `rid.json` |
| [ ] | `oreville_ans:qsqlwc` | instant | partícula `rhd.json` |
| [ ] | `oreville_ans:qsyonu` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:qtgmxm` | instant | partícula `rgr.json` |
| [ ] | `oreville_ans:qturaa` | steady | - |
| [ ] | `oreville_ans:qxfndb` | instant | partícula `reh.json` |
| [ ] | `oreville_ans:qxwqkh` | instant | entidade `minecraft:bogged`, entidade `minecraft:creeper`, entidade `minecraft:enderman`, entidade `minecraft:evocation_illager` e mais 10 |
| [ ] | `oreville_ans:qycthl` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:rakimo` | instant | entidade `minecraft:copper_golem`, entidade `minecraft:creeper` |
| [ ] | `oreville_ans:rbasiz` | manual | partícula `nxc.json` |
| [ ] | `oreville_ans:rbprda` | instant | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:rbpxuq` | instant | partícula `rgg.json`, partícula `rgs.json`, partícula `ril.json`, partícula `riz.json` |
| [ ] | `oreville_ans:reymhz` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:rgtnvf` | instant | partícula `red.json` |
| [ ] | `oreville_ans:rjxqpa` | instant | partícula `mtl.json` |
| [ ] | `oreville_ans:robblt` | steady | attachable `minecraft:soul_torch` |
| [ ] | `oreville_ans:rozsay` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:rpasnl` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:rpccbh` | instant | attachable `minecraft:arrow`, attachable `minecraft:chest_minecart`, attachable `minecraft:cod`, attachable `minecraft:command_block_minecart` e mais 90 |
| [ ] | `oreville_ans:rplzpg` | instant | partícula `rek.json`, partícula `rgf.json`, partícula `rgg.json`, partícula `rig.json` |
| [ ] | `oreville_ans:rqimrp` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:rrcypc` | instant | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:rrqqnv` | instant | partícula `rfu.json` |
| [ ] | `oreville_ans:rufrar` | instant | partícula `rek.json`, partícula `rgf.json`, partícula `rgg.json`, partícula `rig.json` |
| [ ] | `oreville_ans:rujnyp` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:rxlhmw` | steady | - |
| [ ] | `oreville_ans:rzhthr` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:sabqgy` | instant | attachable `minecraft:sugar` |
| [ ] | `oreville_ans:scfubx` | instant | entidade `minecraft:small_fireball` |
| [ ] | `oreville_ans:sfowys` | instant | entidade `minecraft:wither` |
| [ ] | `oreville_ans:sfqtaj` | instant | entidade `minecraft:blaze`, entidade `minecraft:ghast`, entidade `minecraft:hoglin`, entidade `minecraft:magma_cube` e mais 5 |
| [ ] | `oreville_ans:shdqsj` | instant | entidade `minecraft:villager_v2` |
| [ ] | `oreville_ans:shumqi` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:sicdwz` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:sigfni` | steady | attachable `minecraft:ender_pearl`, entidade `minecraft:ender_pearl` |
| [ ] | `oreville_ans:slzkkm` | instant | partícula `riv.json`, partícula `riy.json`, partícula `riz.json` |
| [ ] | `oreville_ans:snwqnx` | instant | attachable `minecraft:pufferfish`, attachable `minecraft:rabbit`, entidade `minecraft:armadillo`, entidade `minecraft:axolotl` e mais 8 |
| [ ] | `oreville_ans:spbywd` | instant | partícula `rgh.json` |
| [ ] | `oreville_ans:swuggf` | steady | entidade `minecraft:player` |
| [ ] | `oreville_ans:szsyfw` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:tbnyfx` | steady | attachable `minecraft:ominous_bottle`, attachable `minecraft:potion` |
| [ ] | `oreville_ans:tbsept` | steady | entidade `minecraft:fishing_hook` |
| [ ] | `oreville_ans:tccqfz` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:tcibuz` | instant | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:tdqvhj` | steady | entidade `minecraft:small_fireball` |
| [ ] | `oreville_ans:tepqaf` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:texrxh` | instant | entidade `minecraft:vex` |
| [ ] | `oreville_ans:tfecoj` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:tfrjam` | instant | entidade `minecraft:armadillo`, entidade `minecraft:bogged`, entidade `minecraft:copper_golem`, entidade `minecraft:creaking` e mais 30 |
| [ ] | `oreville_ans:thndec` | instant | entidade `minecraft:parrot` |
| [ ] | `oreville_ans:tmtnba` | steady | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:tmwdzu` | instant | partícula `rhn.json` |
| [ ] | `oreville_ans:tngokd` | instant | partícula `mtm.json` |
| [ ] | `oreville_ans:torchflower_seeds` | instant | attachable `minecraft:torchflower_seeds` |
| [ ] | `oreville_ans:tsobxp` | steady | entidade `minecraft:iron_golem` |
| [ ] | `oreville_ans:tsvvai` | instant | attachable `minecraft:campfire`, attachable `minecraft:soul_campfire` |
| [ ] | `oreville_ans:ttajna` | steady | entidade `minecraft:wandering_trader`, entidade `minecraft:witch` |
| [ ] | `oreville_ans:tueefs` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:tuembl` | steady | attachable `minecraft:mace` |
| [ ] | `oreville_ans:tvbebm` | steady | entidade `minecraft:allay`, entidade `minecraft:vex` |
| [ ] | `oreville_ans:twdpqw` | instant | entidade `minecraft:wither_skull_dangerous`, entidade `minecraft:wither_skull` |
| [ ] | `oreville_ans:txoeiy` | steady | entidade `minecraft:vex` |
| [ ] | `oreville_ans:tzeeak` | steady | attachable `minecraft:crossbow` |
| [ ] | `oreville_ans:uabixr` | steady | attachable `minecraft:enchanted_golden_apple`, attachable `minecraft:golden_apple` |
| [ ] | `oreville_ans:uaqtdm` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:udehmy` | instant | - |
| [ ] | `oreville_ans:ufxcqk` | instant | partícula `rej.json` |
| [ ] | `oreville_ans:uikxhc` | steady | attachable `minecraft:campfire`, attachable `minecraft:soul_campfire` |
| [ ] | `oreville_ans:ujamov` | steady | entidade `minecraft:sulfur_cube` |
| [ ] | `oreville_ans:ukcozf` | instant | partícula `rgn.json` |
| [ ] | `oreville_ans:ulmjdx` | instant | entidade `minecraft:evocation_illager` |
| [ ] | `oreville_ans:umsrer` | instant | attachable `minecraft:axolotl_bucket`, attachable `minecraft:cod_bucket`, attachable `minecraft:milk_bucket`, attachable `minecraft:pufferfish_bucket` e mais 4 |
| [ ] | `oreville_ans:uodupf` | instant | entidade `minecraft:snow_golem` |
| [ ] | `oreville_ans:usjktb` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:utpepo` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:utqqvt` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:uukqie` | instant | partícula `riv.json`, partícula `riy.json`, partícula `riz.json`, partícula `rjg.json` |
| [ ] | `oreville_ans:uusjml` | instant | partícula `rii.json`, partícula `rik.json`, partícula `ril.json` |
| [ ] | `oreville_ans:uutgiw` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:uuxzsc` | steady | attachable `minecraft:ominous_bottle`, attachable `minecraft:potion` |
| [ ] | `oreville_ans:uzmmel` | instant | attachable `minecraft:chorus_fruit` |
| [ ] | `oreville_ans:vcndke` | instant | entidade `minecraft:turtle` |
| [ ] | `oreville_ans:vdahib` | instant | partícula `rei.json` |
| [ ] | `oreville_ans:vdbzpx` | instant | partícula `rii.json`, partícula `rik.json`, partícula `ril.json`, partícula `riu.json` |
| [ ] | `oreville_ans:vdzssh` | instant | partícula `rgf.json` |
| [ ] | `oreville_ans:vebkux` | steady | partícula `mtm.json` |
| [ ] | `oreville_ans:vetnht` | instant | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:vfxqhr` | instant | attachable `minecraft:arrow`, attachable `minecraft:cod`, attachable `minecraft:egg`, attachable `minecraft:ender_pearl` e mais 15 |
| [ ] | `oreville_ans:vfyiwh` | steady | entidade `minecraft:wind_charge_projectile` |
| [ ] | `oreville_ans:vfyrsi` | steady | attachable `minecraft:spruce_trapdoor` |
| [ ] | `oreville_ans:vjfdfj` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:vkmnfw` | instant | partícula `emd.json` |
| [ ] | `oreville_ans:vlzbjl` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:vmpdgl` | instant | attachable `minecraft:white_candle` |
| [ ] | `oreville_ans:vqcyfx` | instant | entidade `minecraft:breeze` |
| [ ] | `oreville_ans:vrgwcx` | instant | entidade `minecraft:elder_guardian`, entidade `minecraft:sniffer` |
| [ ] | `oreville_ans:vrnmye` | steady | - |
| [ ] | `oreville_ans:vsulpr` | instant | entidade `minecraft:iron_golem`, entidade `minecraft:warden` |
| [ ] | `oreville_ans:vtenjg` | instant | partícula `rgp.json`, partícula `rgr.json`, partícula `rgs.json`, partícula `rgz.json` |
| [ ] | `oreville_ans:vtqqii` | instant | entidade `minecraft:player` |
| [ ] | `oreville_ans:vtviky` | instant | attachable `minecraft:chiseled_stone_bricks` |
| [ ] | `oreville_ans:vudgqo` | manual | partícula `nin.json` |
| [ ] | `oreville_ans:vuullw` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:vvhazw` | instant | partícula `rga.json` |
| [ ] | `oreville_ans:vwuvxm` | instant | partícula `riy.json` |
| [ ] | `oreville_ans:vwzatl` | instant | partícula `rel.json` |
| [ ] | `oreville_ans:vzoaiu` | instant | attachable `minecraft:brown_mushroom` |
| [ ] | `oreville_ans:waekpm` | instant | partícula `ehg.json` |
| [ ] | `oreville_ans:wcqgit` | steady | partícula `rhn.json` |
| [ ] | `oreville_ans:wcueeq` | steady | entidade `minecraft:cat`, entidade `minecraft:wolf` |
| [ ] | `oreville_ans:wdcblg` | instant | entidade `minecraft:spider` |
| [ ] | `oreville_ans:wehlap` | steady | entidade `minecraft:villager_v2` |
| [ ] | `oreville_ans:wffdjp` | instant | entidade `minecraft:vex` |
| [ ] | `oreville_ans:wgjufv` | steady | - |
| [ ] | `oreville_ans:wheat_seeds` | instant | attachable `minecraft:wheat_seeds` |
| [ ] | `oreville_ans:wildflowers` | instant | attachable `minecraft:wildflowers` |
| [ ] | `oreville_ans:wither` | steady | entidade `minecraft:wither` |
| [ ] | `oreville_ans:wnhqgm` | steady | attachable `minecraft:copper_torch`, attachable `minecraft:soul_torch` |
| [ ] | `oreville_ans:wqughz` | steady | entidade `minecraft:wither` |
| [ ] | `oreville_ans:wqxuas` | steady | attachable `minecraft:chicken`, entidade `minecraft:chicken` |
| [ ] | `oreville_ans:wrbaad` | instant | partícula `rhz.json` |
| [ ] | `oreville_ans:wtijvn` | steady | entidade `minecraft:camel_husk`, entidade `minecraft:camel`, entidade `minecraft:cow`, entidade `minecraft:donkey` e mais 12 |
| [ ] | `oreville_ans:wtttth` | instant | partícula `rii.json`, partícula `rik.json`, partícula `ril.json`, partícula `riu.json` |
| [ ] | `oreville_ans:wvqasx` | instant | attachable `minecraft:red_candle` |
| [ ] | `oreville_ans:wvqbjj` | instant | attachable `minecraft:chiseled_stone_bricks` |
| [ ] | `oreville_ans:wwgucn` | instant | entidade `minecraft:evocation_illager`, partícula `rek.json`, partícula `rel.json`, partícula `res.json` e mais 16 |
| [ ] | `oreville_ans:wwvczy` | instant | attachable `minecraft:bow` |
| [ ] | `oreville_ans:wxtpvw` | instant | entidade `minecraft:snow_golem` |
| [ ] | `oreville_ans:wysuyj` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:wzikrc` | instant | partícula `ego.json` |
| [ ] | `oreville_ans:xhybqk` | manual | partícula `nqb.json` |
| [ ] | `oreville_ans:xjpdvo` | instant | partícula `rgn.json` |
| [ ] | `oreville_ans:xjxjzv` | instant | partícula `emq.json` |
| [ ] | `oreville_ans:xkagev` | instant | partícula `egx.json` |
| [ ] | `oreville_ans:xlovmy` | steady | entidade `minecraft:ender_dragon` |
| [ ] | `oreville_ans:xokpsh` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:xp` | steady | entidade `minecraft:xp_orb` |
| [ ] | `oreville_ans:xqtjja` | instant | partícula `riv.json`, partícula `riy.json` |
| [ ] | `oreville_ans:xrnvdt` | instant | entidade `minecraft:creeper` |
| [ ] | `oreville_ans:xsjhvb` | instant | attachable `minecraft:axolotl_bucket`, attachable `minecraft:cod_bucket`, attachable `minecraft:milk_bucket`, attachable `minecraft:pufferfish_bucket` e mais 4 |
| [ ] | `oreville_ans:yarqfa` | instant | attachable `minecraft:red_candle` |
| [ ] | `oreville_ans:ydgaqy` | instant | entidade `minecraft:ravager` |
| [ ] | `oreville_ans:ygqhbg` | steady | entidade `minecraft:fireworks_rocket` |
| [ ] | `oreville_ans:ygumos` | steady | attachable `minecraft:copper_torch` |
| [ ] | `oreville_ans:yhbnex` | instant | partícula `rig.json` |
| [ ] | `oreville_ans:yhejda` | instant | partícula `rfg.json` |
| [ ] | `oreville_ans:yhkynr` | instant | partícula `egq.json` |
| [ ] | `oreville_ans:yhzcxe` | instant | partícula `riv.json`, partícula `riy.json`, partícula `riz.json`, partícula `rjg.json` |
| [ ] | `oreville_ans:yijrou` | instant | entidade `minecraft:breeze`, entidade `minecraft:skeleton`, entidade `minecraft:warden`, entidade `minecraft:wither` |
| [ ] | `oreville_ans:ymalqk` | instant | entidade `minecraft:camel_husk`, entidade `minecraft:camel`, entidade `minecraft:cow`, entidade `minecraft:donkey` e mais 16 |
| [ ] | `oreville_ans:yohytx` | instant | partícula `ept.json` |
| [ ] | `oreville_ans:ypvppc` | instant | entidade `minecraft:wither` |
| [ ] | `oreville_ans:ysntbz` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:ysrjoz` | instant | partícula `ejp.json` |
| [ ] | `oreville_ans:ytfshb` | instant | partícula `rhp.json` |
| [ ] | `oreville_ans:ywkdsq` | instant | partícula `rit.json` |
| [ ] | `oreville_ans:yzrnwe` | instant | entidade `minecraft:snow_golem` |
| [ ] | `oreville_ans:zblnox` | steady | attachable `minecraft:campfire`, attachable `minecraft:soul_campfire` |
| [ ] | `oreville_ans:zdmaqp` | instant | partícula `pqh.json` |
| [ ] | `oreville_ans:zeujtg` | steady | entidade `minecraft:xp_bottle` |
| [ ] | `oreville_ans:zgjzgz` | instant | entidade `minecraft:wither_skeleton` |
| [ ] | `oreville_ans:zhikrm` | instant | partícula `rga.json` |
| [ ] | `oreville_ans:zjohdw` | instant | partícula `eiz.json` |
| [ ] | `oreville_ans:zkluut` | instant | partícula `rgj.json` |
| [ ] | `oreville_ans:zmfipp` | instant | partícula `rfz.json` |
| [ ] | `oreville_ans:znvkrm` | instant | partícula `reg.json` |
| [ ] | `oreville_ans:zuevyd` | instant | attachable `minecraft:mace` |
| [ ] | `oreville_ans:zuzfpp` | instant | entidade `minecraft:iron_golem`, entidade `minecraft:warden` |
| [ ] | `oreville_ans:zwpsyv` | instant | entidade `minecraft:enderman` |
| [ ] | `oreville_ans:zzfcre` | instant | partícula `eln.json`, partícula `eph.json` |
