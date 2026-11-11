# Notas de debug do pack Bedrock (A&S)

Achados reutilizáveis ao depurar o `bedrock-debug.json` (recarrega em ~1 s, sem recompilar).

## Risco branco no rosto (player)

- Sintoma: risco branco/rosa-claro no rosto em 3ª pessoa com o pack A&S.
- Causa: dado da skin, não bug do motor. A camada `geometry.oreville_ans.zwjvro` (expressão do rosto) desenha os olhos como planos 1×1 px que amostram a própria skin.
  - Variante `_1p1`: olho direito na coluna `x=42`, esquerdo em `x=43`, linhas `y=16..19`. Cubos em `hmhadkja89l_1p1` e `hmhadkd67l_1p1`, UV `[42..43, 16..19]` com `uv_size [1,1]`.
  - Região vazia no layout padrão (alpha 0): nada aparece. Pixel opaco ali = olho desenhado (a skin do usuário tinha `(254,230,232,255)` nesses 8 pixels).
- Teste (JSON): `"hide_bones": ["_1p1"]` esconde só essa variante; `"hide_geometry": ["zwjvro"]` esconde a camada inteira.
- Correção: alpha 0 nos pixels `(42..43, 16..19)` da skin.
- Nota: esconder o osso pai (`hmhadk_1p1`) não esconde nada; os cubos ficam nos filhos (`hmhadkja89l_*`, `hmhadkd67l_*`). Usar o sufixo da variante.

### Mapa dos ossos de `zwjvro`

| Osso | O que é |
|---|---|
| `hmhadk_<v>` (pai) / `hmhadkja89l_<v>` / `hmhadkd67l_<v>` | olhos (direito / esquerdo) |
| `_1p1` | 2×2 px |
| `_1p2` | 1×2 px, 3 de largura |
| `_2p1`, `_2p3` | 1×1 px empilhados (2 e 3 alturas) |
| `_3p1`, `_3p2`, `_ea55d6_3p1` | variantes de 3 px com afastamento |
| `_92f5d6` | olho animado (pisca e olhar; animações `fxnhey`, `roymuz`, `lhsngr`…) |
| `6q63jgok*`, `egfg3jgo*` | sobrancelhas (`_1`…`_5` altura; `_2p1`, `_3p1` largura) |
| `324c7246`, `4q4dghk_92f5d6`, `3dafc72467mdd` | planos 8×8 do rosto inteiro |
| `3dafc7246_d67l/_ja89l` | rosto em metades 4×8 |
| `6q6k`, `head`, `body`, `waist`, `root` | transformação, sem cubos |

As variantes não têm `part_visibility`; `ljrhsi` só as move (`v.robswp`, `v.owadjr`).

## Prefixo `oreville`

Prefixo do pack "Actions & Stuff" (`oreville_ans`): ids de geometria, materiais, controladores e opções (`oreville_ans:...`).

## Armadura vestida (worn)

- Ossos modelados em torno da origem (pivô ~`[0,0,0]` Bedrock) caem no pivô do osso de mesmo nome do portador (v97/v98); tolerância 1 px (capacete `[0,-0.2,0]`). Com isso `worn.head/legs/feet` ficam em 0.
- `worn.elytra` move só as asas Java; o elytra do pack segue `worn.chest` (slot `torso`).
- Elytra do pack: `leftwing`/`rightwing` seguem `ElytraAnimationState` do jogo (v99); textura da capa na camada base.
- Couro (A&S, subpack SP2): corpo some. `part_visibility` `{"*": false}, {"d62l96j*": ...}` deixa só os ossos `d62l96j*`; a 2ª textura é pulada (`oreville_wbhprl`/`armor_leather` sem multitexture); a cor do tingimento sai de `q.armor_color_slot` em `v.vishqs/ykbhla/bfcvrw`, que nenhum controlador usa. Pendente.

## Barrel roll do elytra (A&S)

- Só o attachable do elytra (`attachables/fak.json`) o define: controlador `vqnnme` (`pchdlh`), estados `cbixvs` (planando) → `evrjlf` / `fpsgkf` (animações `tfxdcn` / `mluoil`, 2,5 s, `hold_on_last_frame`) com `q.is_gliding && q.is_sneaking && q.is_pack_setting_selected('oreville_ans:fyumxp','hfjbef')`; `hfjbef` é padrão do multiselect.
- As animações giram `head`, `waist`, braços e pernas e batem `left_wing`/`right_wing`. Num attachable só ossos do próprio modelo existem: com `geometry.elytra` (`leftwing`/`rightwing`) nada gira; as asas batem nas geometrias do pack (`uzysde`, `pcbxiq`, camadas de encantamento).
- v99: `is_sneaking` considera a tecla apertada (em voo a pose não é agachada).

## Checklist de validação (v98 em diante)

Antes: recompilar, `worn` zerado no `bedrock-debug.json`, `dump: ["player"]` ligado só para coletar log.

- [ ] v98 capacete comum (ferro, corrente, couro) sem `worn.head.offset`: alinhado na cabeça.
- [ ] v98 capacete de tartaruga sem offset: alinhado (já estava).
- [ ] v98/v97 peitoral, calça e bota sem offset: sem espaço entre as partes, placa da cintura dentro das calças.
- [ ] v100 elytra parado: duas asas lado a lado abrindo para fora em V (15°), sem uma entrar na outra; textura da capa do jogador quando houver capa.
- [ ] v99 elytra agachado em solo: asas abrem (Java: 40°, 45°).
- [ ] v99 elytra planando: asas abertas e presas ao torso.
- [ ] v99 elytra planando + agachar (com `hfjbef` ligado nas opções do pack): a animação de giro dispara; o que se vê depende da geometria (ver acima).
- [ ] Risco branco no rosto: some com alpha 0 nos pixels `(42..43,16..19)` da skin.
- [ ] Estado: ver `bedrock-roadmap.md`, linhas v90–v99, coluna "Validação".

Pendências conhecidas (sem correção):
- Pontos brancos, amarelos e azuis em armaduras, mobs e ferramentas: v101 troca o empurrão por normal por deslocamento da camada inteira em direção à câmera (`lift_toward_camera`); validar em jogo.
- Brilho animado das armaduras (varredura por stencil): as camadas de máscara e de teste "Equal" são puladas desde v93.
- Armadura de couro invisível e tingimento (`q.armor_color_slot` em `v.vishqs`, `v.ykbhla`, `v.bfcvrw`).

Pendências do jogador (v102, validar): bola de canhão ao agachar em queda longa; nado (`is_swimming`) só correndo; vento da elytra (`rujnyp`, controlador `wrhmwv`, exige `is_local_player` e 3ª pessoa); ondulação do nado (`hnsoxk`, exige bloco `water` em (0,-1.5,0)). Lista completa: `docs/bedrock-player-animations.md`.

## Próxima tarefa (depois do nado): borda de encantamento (glint) e creeper carregado

- Objetivo: a borda de encantamento de armaduras encantadas e do creeper carregado (`charged`) fiel ao Bedrock. Hoje o creeper usa uma textura, sem fidelidade.
- Passo 1: identificar as texturas de glint de forma universal (por material/flag e por textura, sem nome de pack), depois testar nos dois casos (armadura encantada e creeper carregado).
- Creeper carregado do Bedrock: rastro de fumaça azul opaca com algumas partículas. Verificar se vem do mesmo tratamento (partículas do `charged` / camada de energia).
- Referência: prints do usuário (armadura de tartaruga e creeper com borda ciano/roxa; creeper com faíscas e fumaça).
- Estado: pendente.
