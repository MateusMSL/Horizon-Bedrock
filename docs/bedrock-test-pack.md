# Horizon Test Pack: por que existe, como é feito e o que já sabemos

Consulta da análise que levou ao pack de teste, das regras do Bedrock descobertas com ele e do
estado da validação. Mapa dos arquivos do motor: `docs/bedrock-arquivos.md`.

## 1. Por que um pack próprio

Até aqui o motor foi validado com o Actions & Stuff (A&S). O A&S tem lógica própria demais
(poses de braço por item, variáveis cruzadas entre jogador e item, animações de equipar), e um
erro do motor fica misturado com um comportamento do pack. Exemplo real: os itens em 1ª pessoa
(picareta abaixo da tela, luneta atrás da câmera, tesoura mostrando os braços). Cada ajuste no
`bedrock-debug.json` acertava um item e estragava outro, e não dava para saber se a culpa era
do motor ou do A&S.

O pack de teste resolve isso: formas simples e cores fixas, cada recurso isolado, o mesmo arquivo
rodando no Bedrock (a referência) e no Horizon (o que se quer validar). Se funciona no Bedrock e
não no Horizon, o defeito é do motor, não do A&S.

## 2. O que o pack testa

| # | Recurso | Como aparece |
|---|---|---|
| 1 | 1ª pessoa, regra pura de ligação na mão | **Papel** vira eixos: +X vermelho, +Y verde, +Z azul, origem branca, cubo amarelo no ponto y = 24 px, haste preta ligando os dois. Sem animação. |
| 2 | 3ª pessoa e 3ª de frente | Os mesmos eixos, vistos pelo F5. |
| 3 | Animação diferente em 1ª e 3ª pessoa | **Graveto**: cubo ciano na mão, haste preta, ponta magenta. 1ª pessoa: inclinado 30°. 3ª pessoa: aponta para a frente. |
| 3b | Malha de textura (`texture_meshes`) | **Tijolo**: sprite de orientação (linha de cima vermelha, coluna da esquerda verde, diagonal branca, bloco azul embaixo à direita) extrudado sem rotação: mostra o plano em que o Bedrock deita a textura. **Sílex**: o mesmo sprite montado como as ferramentas 3D do A&S (osso `sword` dentro de `root`, malha em [-11.31, -11.31, 0.5] girada [-90, 0, -45], osso animado [90, 0, -90] / [0, -2, 4]). |
| 3c | Janela de stencil (como o relógio do A&S) | **Relógio**: moldura amarela; uma janela invisível grava o stencil; o rosto (quadrado magenta 10×10 com centro ciano) só aparece dentro da janela 6×6. Sem stencil, o magenta passa da moldura. |
| 3d | Duas mãos (como a tesoura do A&S) | **Tesoura**: `parent_setup` liga `v.fptest_two_hands` no jogador; em 1ª pessoa os dois braços aparecem, o esquerdo espelhado do direito. |
| 3e | Variável do jogador lida pelo item (como a vara do A&S) | **Vara**: `parent_setup` liga `v.fptest_rod`; o item lê `c.owning_entity->v.fptest_rod` e escala a vara azul por ela (0 some com a vara, só o cubo ciano fica). |
| 3f | Relógio do A&S, material por material | Caixa 10×3×10 (grava 0, topo aberto com borda amarela, fundo marrom fechado), janela 1 px abaixo do topo (grava 3, sem cor), ícones ciano 0,1 px acima da janela e mostrador magenta 1,4 px abaixo dela (só onde há 3). **Bússola**: janela de `entity` com texel transparente (igual ao A&S). **Bússola de recuperação**: janela de `entity` com texel opaco. **Fragmento de eco**: janela de `entity_alphatest` com texel transparente. Serve para descobrir por que, no Bedrock, o mostrador do A&S aparece mesmo atrás da janela. |
| 3g | Partícula de item (attachable) | **Fragmento de ametista**: haste magenta; aura verde a cada 1 s saindo do locator `tip` (ponta de baixo). |
| 3h | Partículas do jogo trocadas pelo pack | O pack traz `minecraft:rain_splash_particle` (magenta), `minecraft:water_splash_particle_manual` (amarelo) e `minecraft:basic_crit_particle` (vermelho): pingos de chuva no chão, respingos de água (nadar, barco, entrar na água) e golpe crítico saem nessas cores, uma partícula por pedido do jogo. |
| 4 | Partícula + som de interação do jogador | Respingo azul ao entrar na água; anel laranja ao pousar depois de mais de 1 s no ar (voo); "blip" nos dois. |
| 5 | Bloco customizado | **Esponja** com textura xadrez magenta/preto (`blocks.json` + `terrain_texture.json`). |
| 6 | Entidade modificada | **Porco** de caixas, cabeça com eixos girando 90°/s. |
| 7 | Partícula de entidade | Aura verde a cada 4 s saindo do locator `top` (coincide com o fim de cada volta da cabeça). |
| 8 | Som | "Blip" do porco 2 s depois de cada aura. |
| 9 | Etiqueta de nome | Porco chamado **Teste** fica ciano (render controller com `q.is_name_any`). |
| 10 | Textos | "Papel (eixos de teste)", "Graveto (animação de teste)", "Porco (teste)", "Esponja (teste)". |

## 3. Como o pack é gerado

- Gerador: `tools/test-pack/build_test_pack.py`.
  `python tools/test-pack/build_test_pack.py "<Minecraft for Windows>\Content\data" saida.mcpack`
- O pack precisa trazer um **jogador Bedrock**: o Horizon só troca o item da mão pelo modelo do
  pack quando o próprio jogador é desenhado por um pack. Sem isso ele desenha jogador e item no
  estilo Java e o teste não aparece.
- O jogador vanilla (entidade, animações, controladores, render controllers e modelos) é lido da
  instalação do Bedrock (`resource_packs/vanilla/__brarchive/*.brarchive`) na hora de gerar. Esses
  arquivos são da Mojang e não ficam no repositório; só o gerador fica.
- Ao jogador são acrescentados os eventos do teste (controlador
  `controller.animation.fptest.player_events`, partículas `fptest_splash`/`fptest_land`, som
  `fptest_blip`).
- Texturas são paletas geradas no script (um texel por cor); o som é gerado com `ffmpeg`.
- UUID fixo; cada mudança sobe a versão do manifest para o Bedrock substituir a anterior ao
  importar.

### Conferir sem abrir o jogo

`cargo run --release -p horizon-bedrock --example test_pack -- horizon-test-pack.mcpack`
converte o pack como o Horizon faz, monta o `Bundle` e imprime `ok`/`FALHOU` para cada recurso
(attachables, modelos, animações, jogador e eventos, porco, partículas, som, bloco). Duas
texturas aparecem como faltando e são esperadas: `steve` (no jogo vira a skin do jogador) e
`cape_invisible`.

## 4. Regras do Bedrock descobertas

| Regra | Como descobrimos | Impacto no Horizon |
|---|---|---|
| **Item na mão:** o ponto y = 24 px do modelo do attachable fica sobre o osso da mão; a origem do modelo (pés, y = 0) fica 1,5 bloco "abaixo" no eixo do braço. Em 1ª pessoa o eixo +Y aponta de volta para a mão e a origem fica longe, à frente e acima. | Teste 1 no Bedrock (cubo amarelo na mão, origem branca longe). Bate com a nota do mcblend ("Minecraft adds [0, -1.5, 0] offset"). | Base para corrigir a âncora em 1ª pessoa (`render_03.rs`, `fp_held_anchor`). |
| **Modelo do item deve partir da mão** (y = 24) se for girar em volta dela; um modelo na origem gira em volta dos pés e some. | Graveto: na 1.0.0 sumia em 3ª pessoa; na 1.0.2 (modelado em y = 24 descendo) aparece nas três vistas. | Só design do pack. |
| **Bounds visíveis:** o Bedrock deixa de desenhar o modelo cujo `visible_bounds` sai da vista. | Graveto de frente sumia com bounds 4×4; com 8×8 aparece. | O Horizon não recorta por bounds hoje (não precisa imitar o defeito). |
| **Definições repetidas de entidade:** o Bedrock fica com a do `format_version` de arquivo mais novo, independente da ordem dos packs. `min_engine_version` não decide. | Porco: com 1.10.0 (igual ao vanilla) e depois com `min_engine_version` 1.26.60, o vanilla ganhava; com `format_version` 1.26.30 (o que o A&S usa) o pack ganha. | **A conferir** no `bundle.rs`: o Horizon hoje decide pela ordem dos packs. |
| **Pouso:** o estado "no ar" sempre ocorre num pulo; para pegar só queda/voo é preciso tempo no ar (`q.state_time > 1.0`). | Teste 9: na 1.0.0 soltava a cada pulo. | Só design do pack. |
| **`blocks.json` antigo:** com `format_version` anterior ao achatamento (`[1, 1, 0]`), o nome legado vale para todos os blocos em que ele virou: `sponge` muda também a esponja molhada. | Teste 5: no Bedrock as duas esponjas ficaram xadrez. | `legacy_blocks` em `convert.rs` (v205). |
| **Content Log** (Configurações → Criador) mostra erros de pack; o porco não gerava erro, só perdia para o vanilla. | Investigação do porco. | Ferramenta de diagnóstico. |

## 5. Histórico do pack

| Versão | Mudança |
|---|---|
| (antes) `fp-axes-test` | Só os eixos no papel. Perdia para o A&S (que também redefine o papel). |
| (antes) `fp-axes-vanilla` | Eixos + jogador vanilla. Primeiro teste limpo no Bedrock. |
| 1.0.0 | Pack completo (papel, graveto, eventos do jogador, porco, esponja, sons, textos). |
| 1.0.1 | Porco com o caminho do vanilla e textura por array; pouso só depois de 1 s no ar; graveto modelado na mão. |
| 1.0.2 | Graveto descendo da mão (aparece em 1ª pessoa). |
| 1.0.3 | Bounds maiores (graveto de frente); porco com `min_engine_version` (não resolveu). |
| 1.0.4 | Ovelha no lugar do porco (tentativa, revertida). |
| 1.0.5 | Porco com `format_version` 1.26.30: funciona. |
| 1.0.6 | Tijolo e sílex com `texture_meshes` (isolar a picareta torta do A&S: textura carregava, a malha ficava em outro plano/ângulo). |
| 1.0.7 | Relógio (stencil), tesoura (duas mãos), vara (variável do jogador): os três defeitos do A&S que sobraram (rosto do relógio some, braços gigantes na câmera, vara invisível). |
| 1.0.8 | No Bedrock a 1.0.7 falhou nos três: relógio só moldura (janela grossa na frente do rosto tapava pela profundidade), tesoura sem braços e vara sem a haste (variáveis do jogador zeradas a cada quadro e não públicas). Agora como o A&S: rosto antes da janela, planos finos; variáveis zeradas só no `initialize` e declaradas `public`. |
| 1.0.9 | Réplicas do relógio do A&S (bússola, bússola de recuperação, fragmento de eco) para isolar a regra da janela. |
| 1.0.10 | Partícula de item (fragmento de ametista) e partículas do jogo trocadas pelo pack (chuva, respingo, crítico). |

## 6. Resultado no Bedrock (1.0.5)

| # | Teste | Bedrock |
|---|---|---|
| 1–2 | Papel 1ª / 3ª / 3ª de frente | ok |
| 3 | Graveto nas três vistas | ok (de frente só a ponta aparece, o resto fica atrás do corpo) |
| 4 | Água e pouso | ok |
| 5 | Esponja | ok |
| 6–8 | Porco, aura, som | ok |
| 9 | Etiqueta "Teste" | ok |
| 10 | Nomes | ok |

## 7. Resultado no Horizon

| # | Teste | v202 | v203 | v204 |
|---|---|---|---|---|
| 1–2 | Papel 1ª / 3ª / 3ª de frente | 1ª: papel Java; 3ª: invertido (amarelo 1,5 bloco acima da mão) | aparece nas três; amarelo na mão; ângulos um pouco diferentes do Bedrock | 1ª: mais baixo e menor que no Bedrock (braço 10 px abaixo): v206 |
| 3 | Graveto nas três vistas | 1ª: graveto Java; 3ª ok | aparece nas três; ângulos um pouco diferentes | 1ª: idem papel: v206 |
| 3b | Tijolo / sílex (`texture_meshes`) | — | — | tijolo ok; sílex deslocado (v208) |
| 3c–3e | Relógio / tesoura / vara (1.0.8) | — | — | relógio só moldura (Bedrock: moldura + ciano) → v210; tesoura e vara iguais ao Bedrock |
| — | Ovo do porco (`spawn_egg` com cores) | — | — | ícone padrão; no Bedrock vira o ovo de pintas: v206 |
| 4 | Água e pouso | ok | ok | — |
| 5 | Esponja | textura vanilla | textura vanilla | seca ok; molhada vanilla (no Bedrock também muda): v205 |
| 6–8 | Porco, aura, som | ok (blip 2 s depois da aura, como no pack) | ok | — |
| 9 | Etiqueta "Teste" | ok | ok | — |
| 10 | Nomes | padrão do jogo | padrão do jogo | ok |
| — | Braços e pernas do jogador | — | — | giravam demais (`v.gliding_speed_value` = velocidade): v205 |

Causas corrigidas:
- **v203:** o jogador vanilla esconde todas as partes em 1ª pessoa ao segurar item (`*: false`),
  e o Horizon perdia junto os ossos da mão; o osso do attachable preso à mão usava a origem do
  modelo em vez do ponto y = 24.
- **v204:** `blocks.json` com outro nome de textura e `texts/*.lang` não eram convertidos.
- **v205:** `v.gliding_speed_value` valia a velocidade (≈0 parado) e o balanço de andar do
  jogador vanilla dividia por ele; `blocks.json` antigo (`[1, 1, 0]`) vale para todos os blocos
  em que o bloco legado virou (esponja seca e molhada).
- **v206:** `v.player_arm_height` não existia (valia 0) e o `swap_item` vanilla baixava o braço
  10 px em 1ª pessoa; `spawn_egg` do client entity não era convertido.
- **v207:** em 1ª pessoa o modelo era escalado (0,9375) em volta dos pés; o Bedrock mantém o olho
  na câmera, e o item ficava ~0,1 bloco mais baixo (mão fora da tela).
- **v208:** tijolo igual ao Bedrock (plano da malha certo); sílex deslocado ao lado do corpo e fora
  da tela em 1ª pessoa: o y de `position` da malha conta para baixo a partir do pivô do osso
  (regra do Blockbench), não para cima.

## 8. Contexto: a investigação de 1ª pessoa que motivou o pack

Resumo do que já foi tentado no motor (detalhes em `docs/bedrock-roadmap.md`):

- **v197–v198, v201:** logs `fp-attach` e `fp-view` (posição do osso ligado e dos vértices do item
  no espaço da câmera, 1 por segundo com `trace_fp`).
- **v199:** âncora `frame` em 1ª pessoa: errado (tesoura acima da câmera).
- **v200:** âncora `holder` (regra de osso filho) em 1ª pessoa: errado sozinho; é o padrão atual,
  com `fp_held_anchor` no JSON para trocar.
- **v202:** `fp_rot_signs` para testar sinais de rotação: a convenção de rotação do motor está
  certa (confere com o código do Blockbench).
- Com o A&S, descontar a animação de equipar (`rikfbu`) do braço traz itens para a tela, mas ainda
  pequenos e deitados. Por isso a decisão de validar com o pack de teste, sem o A&S.

## 9. Variantes do relógio do A&S (`horizon-clock-variants.mcpack`)

Gerador: `python tools/test-pack/build_clock_variants.py saida.mcpack`. Pack separado (carregar
sozinho no Bedrock). Cada item é uma cópia do relógio 3D do A&S (mesma geometria, mesmo lugar na
textura, mesma cadeia de materiais, attachable 1.26.30, animações de 1ª/3ª pessoa) com uma
diferença. O mostrador é o quadrado magenta com cruz amarela dentro da borda dourada.

| Item | Variante |
|---|---|
| Relógio | V0: igual ao A&S |
| Bússola | V1: attachable `format_version` 1.10.0 |
| Bússola de recuperação | V2: materiais com `FANCY` |
| Fragmento de eco | V3: sem animação de 1ª pessoa |
| Fragmento de ametista | V4: sem osso `root` (item no topo) |
| Fragmento de prismarinho | V5: janela de `entity_alphatest` |
| Concha de náutilo | V6: janela antes do mostrador na lista de ossos |
| Coração do mar | V7: no render controller, janela e mostrador antes de `*` |
| Lágrima de ghast | V8: caixa sem stencil |
| Vara de blaze | V9: janela abaixo do mostrador |
| Estrela do Nether | V10: sem janela |

Leitura: se V0 mostra o mostrador e uma variante não, aquela diferença é a regra. Se V10 mostra o
mostrador, o teste `Equal 3` não está barrando nada (stencil de quadros anteriores ou ignorado).
