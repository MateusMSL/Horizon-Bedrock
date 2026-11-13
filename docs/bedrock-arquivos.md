# Bedrock no Horizon: mapa dos arquivos

O que cada arquivo do suporte a add-ons Bedrock faz, por onde passa um pack do disco até a tela,
e onde mexer para cada tipo de problema. Lista dos ganchos em arquivos originais do Horizon:
`docs/bedrock-hooks.md`.

## O caminho de um pack

```
addons/*.mcpack ──► horizon-client-render/src/packs/bedrock.rs   (descobre, converte, guarda em cache)
                        │
                        ▼
              horizon-bedrock/src/convert.rs                     (Bedrock ➜ pack Java em disco:
                        │                                          texturas PNG, sons, lang, JSON do pack)
                        ▼
              horizon-bedrock/src/bundle.rs                      (carrega o que foi convertido:
                        │                                          entidades, attachables, modelos,
                        │                                          animações, partículas, materiais)
                        ▼
              horizon-client/src/mobs/bedrock.rs                 (a cada quadro, por mob: roda Molang,
                        │                                          animações e controladores ➜ MobFrame)
                        ▼
              horizon-client/src/mobs/bedrock_render/*.rs        (MobFrame ➜ vértices: corpo, camadas,
                        │                                          materiais, itens na mão, 1ª pessoa)
                        ▼
              horizon-client/src/game/bedrock.rs                 (toca sons e desenha partículas que
                                                                   os quadros pediram)
```

Regra geral: o pack só decide **o que aparece e o que toca**. A lógica do jogo (vida, IA,
física, inventário) continua sendo a do Java; o motor Bedrock só lê o estado do mob.

## Crate `crates/horizon-bedrock` — ler e entender o pack (sem desenhar nada)

| Arquivo | Responsável por |
|---|---|
| `src/lib.rs` | Porta de entrada do crate; lista os módulos. |
| `src/pack.rs` | Abre pasta, `.mcpack` ou `.mcaddon` como uma árvore de arquivos; resolve subpacks (o que o jogo carregaria). |
| `src/manifest.rs` | `manifest.json`: nome, UUID, módulos, subpacks. |
| `src/settings.rs` | Opções do pack (`settings` do manifest: toggles, dropdowns) e as respostas de `q.is_pack_setting_enabled/selected`. |
| `src/jsonc.rs` | JSON do Bedrock (comentários, vírgula sobrando, BOM) ➜ JSON normal. |
| `src/brarchive.rs` | Formato `.brarchive` (vários JSON num arquivo, usado pelo vanilla do Bedrock). |
| `src/convert.rs` | Converte o pack Bedrock num pack Java em disco: texturas (`textures/...` ➜ `entity/bedrock/...`, itens ➜ `item/...`), sons, `blocks.json` (textura trocada ➜ `block/<id>`), `texts/*.lang` ➜ `lang/*.json` (nomes de item, bloco, entidade), cópia dos JSON que o motor lê. |
| `src/bundle.rs` | Lê o pack convertido para a memória (`Bundle`): entidades, attachables, geometrias, animações, partículas, materiais, sons. Empilha vários packs (o de cima vence) via `order.txt`. Ajusta os sprites de `texture_meshes` ao tamanho real das texturas. |
| `src/geometry.rs` | Modelos (`models/entity/*.json`): ossos, cubos, UVs (com `uv_rotation`), `texture_meshes` (itens 2D virando caixinhas), pivôs. Converte para o formato de partes do Java. |
| `src/entity.rs` | Client entities, attachables e render controllers: qual modelo/textura/material, quais animações rodam, `pre_animation`, `parent_setup`, part_visibility. Monta a pose do quadro (`Frame`), resolve rotações `relative_to entity` e as consultas `q.bone_origin`/`bone_orientation_trs`. |
| `src/animation.rs` | Animações e controladores de animação: keyframes (rotação/posição/escala, números ou Molang), loops, blend, estados e transições, sons/partículas/scripts com tempo. |
| `src/molang/*.rs` | A linguagem Molang: `lexer` (texto ➜ tokens), `parser` (tokens ➜ programa), `ast` (forma do programa), `eval` (executa; variáveis `v.`, temporárias `t.`, `q.` respondidas pelo jogo), `math` (`math.*` em graus), `tests`. |
| `src/material.rs` | `materials/*.material`: de qual material vanilla deriva e o que muda (alpha test, blend, emissivo, sem depth write, `color_mask`...). |
| `src/particle.rs` | Partículas (`particles/*.json`): leitura dos componentes e a simulação na CPU (emissor, vida, movimento, cor, flipbook). |
| `src/sounds.rs` | `sound_definitions.json` e `sounds.json`: sons nomeados, variações, e troca de sons de eventos de entidade. |
| `src/lang.rs` | `texts/*.lang` (nomes e textos traduzidos). |
| `src/world_data.rs` | Arquivos do mundo em volta: névoa (`fogs/`), biomas do cliente, `blocks.json`, culling de blocos. |
| `src/audit.rs` | Conta o que o pack tem e o motor ainda não lê (para saber o que falta implementar). |
| `examples/*.rs` | Ferramentas de linha de comando, rodam fora do jogo: `convert_pack` (converte), `scan_pack` (lê tudo e acusa erro de JSON/Molang), `audit_pack`/`audit_attachables` (o que falta/está quebrado), `inspect_entity`/`probe_*` (camadas, poses, partículas de uma entidade), `render_entity` (desenha a entidade em PNG, sem o jogo), `validate_install`, `check_textures`, `list_glow`. Uso: `cargo run --release -p horizon-bedrock --example <nome> -- <pack> ...` |

## Cliente `crates/horizon-client` — aplicar o pack aos mobs do jogo

| Arquivo | Responsável por |
|---|---|
| `src/mobs/bedrock.rs` | O coração em tempo de execução. `BedrockMobs::load` carrega o `Bundle`; `frame()` roda, por mob e por quadro, o Molang, as animações e os controladores do pack e devolve um `MobFrame` (pose dos ossos, camadas a desenhar, itens presos — `Attached` —, sons e partículas emitidos). Responde as consultas `q.*` com o estado do mob Java (`MobHost`, `AttachHost`). Também: `ENGINE_BUILD` (versão do motor no log), `apply_pose` (pose Bedrock ➜ partes Java), superfícies em volta (`Surroundings`), partículas do pack. |
| `src/mobs/bedrock_debug.rs` | O `bedrock-debug.json` (pasta `%APPDATA%\horizon`): chaves de depuração recarregadas ao salvar, sem compilar (esconder ossos, mover/girar ossos, forçar materiais, dumps no log, `trace_fp`, `fp_offset`, `held_anchor`...). Cada campo está comentado no próprio arquivo. |
| `src/mobs/render.rs` | Arquivo original do Horizon; inclui os 4 arquivos abaixo com `include!` e tem as funções de matriz usadas por eles (`held_anchor_at`, `rest_pivot`, `posed`). |
| `src/mobs/bedrock_render/render_01.rs` | Agrupa os ossos de uma camada por material (uma passada de desenho por "look"), luz achatada (`filter_lighting`), UV animado (`uv_anim`). |
| `src/mobs/bedrock_render/render_02.rs` | Marca os vértices de cada camada com o tipo de desenho que o material pede (blend, sem depth write, sem depth test, alpha; `ignores_alpha` inclui `color_mask`). |
| `src/mobs/bedrock_render/render_03.rs` | O desenho em si: `render_bedrock` (cada camada do render controller, posada, com textura do atlas; armaduras e itens presos — attachables — no esqueleto do mob, âncora do item na mão), `first_person_item` (a mão e o item em **1ª pessoa**: desenha uma cópia do jogador presa à câmera), logs `fp-trace`/`fp-attach`/`fp-view`. |
| `src/mobs/bedrock_render/render_04.rs` | Base do modelo do pack em relação ao modelo do jogo, onde o pack segura um item (`bedrock_hand`), posição de um osso do pack para outros sistemas (`bedrock_bone_frame`), se o attachable substitui o item do jogo. |
| `src/mobs/bedrock_stencil.rs` | **Stencil do Bedrock (lado do mob).** Decide o destino de cada peça cujo material usa stencil (`EnableStencilTest`): só a cena, cena + passe de stencil (grava valor com cor) ou só o passe (testa o valor, ou máscara sem cor). `emulated` desliga as imitações de CPU quando o passe está ligado; `hand_over` entrega os vértices. Fora do passe (boneco do inventário, shader pack) as imitações antigas continuam. |
| `src/game/bedrock.rs` | Toca os sons e desenha as partículas que os quadros dos mobs pediram. |
| `src/game/pack_proxy.rs` | Coisas que o jogo desenha sozinho (suporte de armadura, carrinho, projéteis, dragão) desenhadas com o modelo do pack: um "mob substituto" roda as animações do pack. |

## Render `crates/horizon-client-render` — packs e telas

| Arquivo | Responsável por |
|---|---|
| `src/packs/bedrock.rs` | Pasta `addons/`: descobre os packs Bedrock, converte (via `convert.rs`) e guarda a conversão em cache (`.horizon-cache`), refaz só quando o arquivo muda; subpack escolhido. |
| `src/gfx/bedrock_stencil.rs` | **Stencil do Bedrock (lado da GPU).** Passe próprio com buffer de profundidade+stencil (`Depth32FloatStencil8`, ou `Depth24PlusStencil8`): copia a profundidade da cena, desenha as peças na ordem do modelo com o teste/gravação de stencil do material (`State`), com o shader de entidades (`fs_entity_uncut` sem corte de alfa para material sem `ALPHA_TEST`). Um passe depois do mundo (`draw_world`) e um depois da mão em 1ª pessoa (`draw_hand`). Pipelines criados sob demanda por estado. |
| `src/pack_screen.rs` (gancho) | Tela de add-ons: lista, ordem, botão **Settings** (opções e subpacks do pack). |

## Documentos e ferramentas do repositório

| Arquivo | Para quê |
|---|---|
| `docs/bedrock-arquivos.md` | Este mapa. |
| `docs/bedrock-hooks.md` | Lista dos arquivos originais do Horizon que recebem ganchos (e quantas linhas). |
| `docs/bedrock-roadmap.md` | Histórico de cada versão do motor (`ENGINE_BUILD`): o que mudou e por quê. |
| `docs/bedrock-instalacao.md` | Compilar, importar no launcher, instalar add-ons (dev). |
| `docs/bedrock.md`, `bedrock-status.md`, `bedrock-coverage.md` | Visão geral, estado e cobertura do formato Bedrock. |
| `docs/bedrock-debug-notes.md`, `bedrock-first-person-tuning.md`, `bedrock-player-animations.md`, `bedrock-items-3d.md`, `bedrock-itens-checklist.md` (gerado por `tools/bedrock/items_checklist.py`), `bedrock-validation.md` | Notas de depuração de temas específicos. |
| `tools/check-hooks.ps1` + `tools/bedrock-hooks.txt` | Confere, depois de atualizar do upstream, se os ganchos continuam nos arquivos originais. |
| `patches/bedrock-hooks.patch` | Reaplica ganchos perdidos (`git apply --3way`). |
| `tools/fp-test/fp-axes-test.mcpack` | Pack de teste antigo (só os eixos no papel). |
| `tools/test-pack/build_test_pack.py` | Gera o **Horizon Test Pack** (`horizon-test-pack.mcpack`): papel = eixos X/Y/Z (regra de ligação na mão), graveto = animação diferente em 1ª e 3ª pessoa, partículas e som ao entrar na água e ao pousar, esponja com textura xadrez, porco com modelo/partículas/som próprios e azul-ciano quando chamado "Teste", nomes trocados. O jogador vanilla vem da instalação do Bedrock (não fica no repo). Uso: `python tools/test-pack/build_test_pack.py "<Minecraft for Windows>/Content/data" saida.mcpack` |
| `crates/horizon-bedrock/examples/test_pack.rs` | Confere o Horizon Test Pack como o jogo carrega (converte, monta o bundle, checa cada recurso): `cargo run --release -p horizon-bedrock --example test_pack -- horizon-test-pack.mcpack` |
| `packaging/windows/LEIA-ME.md` | Instruções para o usuário final do zip. |
| `docs/renderdoc.md` | Como capturar um quadro do Bedrock e do Horizon no RenderDoc e o que olhar (ordem dos desenhos, estado de stencil/profundidade, histórico de pixel). |

## Ganchos em arquivos originais (resumo do papel de cada um)

| Arquivo | O gancho serve para |
|---|---|
| `Cargo.toml`, `*/Cargo.toml` | Incluir o crate `horizon-bedrock`. |
| `horizon-client/src/mobs.rs`, `mobs/data.rs` | Declarar os módulos Bedrock e guardar `BedrockMobs` nos dados dos mobs. |
| `horizon-client/src/mobs/render.rs` | Chamar o desenho Bedrock antes do modelo vanilla (e cair no vanilla quando o pack não desenha). |
| `horizon-client/src/game.rs`, `game/packs.rs` | Carregar o pack ao trocar recursos; fogo do pack no lugar do vanilla. |
| `game/perspective.rs` | Dar ao pack as superfícies em volta do mob (água, chão) que o Molang consulta. |
| `game/frame_geometry.rs`, `item_render.rs` | Itens na mão e em 1ª pessoa: deixar o attachable do pack substituir o item do jogo. |
| `game/armor_stands.rs`, `minecarts.rs`, `dragon.rs`, `native_projectiles.rs`, `sulfur_cubes.rs`, `mob_blocks.rs` | Entidades que o jogo desenha sozinho, desenhadas pelo pack via `pack_proxy.rs`. |
| `game/passengers.rs`, `mounts.rs`, `remote_vehicles.rs`, `mobs/riders.rs`, `mobs/camel.rs` | Montaria/passageiro: tipo do veículo e altura do assento para o pack. |
| `game/remote.rs`, `mobs/nether.rs` | Estado extra que o pack consulta (ex.: blaze carregado). |
| `game/creatures/mob_geometry.rs`, `mobs/skin.rs`, `mobs/cem*.rs`, `random_textures.rs` | Convivência com skins/CEM do Java quando o pack veste o jogador ou o mob. |
| `horizon-client-render/src/packs.rs`, `options_ui*.rs`, `game/options.rs`, `startup_menu.rs`, `lang/*.json` | Página **Add-ons** nas opções, pasta `addons/`, textos. |
| `horizon-client-render/src/gfx.rs`, `gfx/entity_batch.rs`, `entity.wgsl`, `hud.rs` | Modos de desenho que os materiais Bedrock precisam (aditivo, sem depth, OIT, corte de alpha; `fs_entity_uncut` do passe de stencil) e texturas `entity/bedrock/` no atlas. |
| `horizon-client-render/src/gfx.rs`, `gfx/surface_setup.rs` (stencil) | Campo `bedrock_stencil` no `Renderer`; recurso `DEPTH32FLOAT_STENCIL8` no device; `begin_frame` e guardar a profundidade do mundo quando há peças; `draw_world` depois do passe do mundo; mão fora do passe do mundo e profundidade da mão guardada quando há peças; `draw_hand` depois da mão. |
| `horizon-assets/src/item_model*.rs` | Leitura de componentes de item que os packs consultam. |

## Onde mexer para cada problema

| Sintoma | Comece por |
|---|---|
| Pack não carrega / textura faltando | `packs/bedrock.rs`, `convert.rs`; log `converted the Bedrock pack`; exemplo `scan_pack`. |
| Modelo deformado / UV errado | `geometry.rs`; exemplo `render_entity`. |
| Animação errada (osso no lugar errado, controlador preso) | `animation.rs`, `entity.rs`; `bedrock-debug.json` `dump` + `dump_bones`. |
| Expressão Molang falha / consulta `q.x` sem resposta | `molang/*`, `MobHost::query` em `mobs/bedrock.rs` (log lista consultas sem resposta). |
| Material (brilho, transparência) errado | `material.rs`, `render_01.rs`, `render_02.rs`; `bedrock-debug.json` `materials`. |
| Item na mão (3ª pessoa) fora do lugar | `render_03.rs` (`base_of`, âncora `held_anchor`), `render_04.rs`. |
| Item/mão em **1ª pessoa** | `render_03.rs` `first_person_item` e o bloco `fp_held_anchor`; `bedrock-debug.json` `trace_fp`, `fp_offset`; log `fp-view`. |
| Stencil (relógio sem mostrador, brilho de armadura fora da faixa, contorno) | `material.rs` (`Stencil`), `mobs/bedrock_stencil.rs`, `gfx/bedrock_stencil.rs`. |
| Partícula | `particle.rs`, `game/bedrock.rs`. |
| Som | `sounds.rs`, `game/bedrock.rs`. |
| Entidade que o jogo desenha sozinho | `game/pack_proxy.rs` e o gancho do arquivo da entidade. |
