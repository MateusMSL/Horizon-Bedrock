# Itens com modelo 3D (attachables do A&S) e o que o motor lê

Medido por `cargo run --release -p horizon-bedrock --example audit_attachables -- <pack>`: cada attachable é avaliado uma vez como item na mão (40 ticks). Dono sempre `is_owner_identifier_any=1`, sem nome, sem alvo.

## Totais

| Medida | Valor |
| --- | --- |
| Attachables | 1699 |
| Com geometria de cubos | 1697 |
| Avaliados com osso visível, geometria e textura | 1615 (95%) |
| Com problema na avaliação | 84 (5%) |

## Contextos em que o motor usa o attachable

- [x] Item na mão em 3ª pessoa (espada, besta, tridente, mace, bola de vento, ovos)
- [~] Item na mão em 1ª pessoa (sem animação de ataque; pose da mão do jogo)
- [~] Armadura vestida (osso do portador)
- [ ] Item caído no chão
- [ ] Item em moldura
- [ ] Inventário, hotbar, GUI (`is_in_ui`, `is_paperdoll`)
- [ ] Entidade no mundo com o modelo do pack: minecart (+ baú, funil, TNT, bloco de comando), barcos (modelo só nos attachables dos itens), flecha, tridente arremessado, bolas de fogo, caveira do wither, bola de neve, ovo, pérola, olho de ender, foguete
- [ ] Bloco colocado no mundo com o modelo do item (alavanca, lanterna, sino, bigorna, cama, baú, placa, banner, etc.: o pack só os traz como item)
- [ ] Armor stand do pack (armaduras do pack, animação colocado no chão)

## Itens do pack sem leitura completa

### Geometria do jogo ausente no pack (2)
- [ ] `elytra`
- [ ] `shield`

### Textura do jogo ausente no pack (6)
- [ ] `creeper_head`: `textures/entity/skulls/creeper.png`
- [ ] `poplar_sapling`: `textures/blocks/poplar_sapling.png`
- [ ] `skeleton_skull`: `textures/entity/skulls/skeleton.png`
- [ ] `tadpole_bucket`: `textures/entity/tadpole/tadpole.png`
- [ ] `wither_skeleton_skull`: `textures/entity/skulls/wither_skeleton.png`
- [ ] `zombie_head`: `textures/entity/skulls/zombie.png`

### Dependem do dono ou do slot: o teste os marca sem camada (46)
Tapetes, arreios, bolos e blocos que o pack só mostra com uma condição (cavalo, lhama, cubo de enxofre, nome); conferir no jogo.
- [ ] `black_carpet`
- [ ] `black_harness`
- [ ] `blue_carpet`
- [ ] `blue_harness`
- [ ] `brown_carpet`
- [ ] `brown_harness`
- [ ] `cake`
- [ ] `carrot`
- [ ] `cobblestone`
- [ ] `cookie`
- [ ] `crimson_fungus`
- [ ] `cyan_carpet`
- [ ] `cyan_harness`
- [ ] `gray_carpet`
- [ ] `gray_harness`
- [ ] `green_carpet`
- [ ] `green_harness`
- [ ] `heartbreak_pottery_sherd`
- [ ] `lava_bucket`
- [ ] `light_blue_carpet`
- [ ] `light_blue_harness`
- [ ] `light_gray_carpet`
- [ ] `light_gray_harness`
- [ ] `lime_carpet`
- [ ] `lime_harness`
- [ ] `magenta_carpet`
- [ ] `magenta_harness`
- [ ] `moss_carpet`
- [ ] `mossy_cobblestone`
- [ ] `orange_carpet`
- [ ] `orange_harness`
- [ ] `pale_moss_carpet`
- [ ] `pink_carpet`
- [ ] `pink_harness`
- [ ] `purple_carpet`
- [ ] `purple_harness`
- [ ] `red_carpet`
- [ ] `red_harness`
- [ ] `snow_layer`
- [ ] `stone_bricks`
- [ ] `warped_fungus`
- [ ] `white_carpet`
- [ ] `white_harness`
- [ ] `wolf_armor`
- [ ] `yellow_carpet`
- [ ] `yellow_harness`

### Armaduras `.player` (slot vestido; teste na mão não as mostra) (29)
- [ ] `chainmail_boots.player`
- [ ] `chainmail_chestplate.player`
- [ ] `chainmail_helmet.player`
- [ ] `chainmail_leggings.player`
- [ ] `copper_boots.player`
- [ ] `copper_chestplate.player`
- [ ] `copper_helmet.player`
- [ ] `copper_leggings.player`
- [ ] `diamond_boots.player`
- [ ] `diamond_chestplate.player`
- [ ] `diamond_helmet.player`
- [ ] `diamond_leggings.player`
- [ ] `golden_boots.player`
- [ ] `golden_chestplate.player`
- [ ] `golden_helmet.player`
- [ ] `golden_leggings.player`
- [ ] `iron_boots.player`
- [ ] `iron_chestplate.player`
- [ ] `iron_helmet.player`
- [ ] `iron_leggings.player`
- [ ] `leather_boots.player`
- [ ] `leather_chestplate.player`
- [ ] `leather_helmet.player`
- [ ] `leather_leggings.player`
- [ ] `netherite_boots.player`
- [ ] `netherite_chestplate.player`
- [ ] `netherite_helmet.player`
- [ ] `netherite_leggings.player`
- [ ] `turtle_helmet.player`

### Outros (1)
- [ ] `oreville_ans:lollng`

## Problemas conhecidos nos itens

- Sem animação de ataque em 1ª pessoa
- Ataque em 3ª pessoa executa duas animações por clique (espada: 1+2, depois 3+1; tridente duas vezes)
- Itens 2D (`texture_meshes`) com bug de textura (capturas de 2026-11-09: faixa de cor, textura fora do lugar, item caído deformado)
- Bola de vento mal renderizada na mão
