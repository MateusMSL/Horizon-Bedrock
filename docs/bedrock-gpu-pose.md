# Mobs Bedrock com pose na GPU (análise)

Branch `bedrock-gpu-pose`. Análise feita antes de implementar: o que o caminho de pose na GPU
dos mobs vanilla já oferece, o que os modelos dos packs Bedrock precisam além disso, as decisões
de arquitetura e a ordem de implementação.

## Por que

Fazenda de golems de cobre num servidor (202 golems a 64 blocos, ~94 desenhados por quadro),
build release, medido pelo socket de controle:

| Por quadro | Sem add-on | Com Actions & Stuff (após as otimizações de CPU) |
|---|---|---|
| FPS | 239–315 | 88–110 |
| Mobs (`eg_mobs`) | 0,46 ms | 4,5 ms |
| Envio para a GPU (`renderer`) | 1,8 ms | 3,2 ms |

Os mobs vanilla usam a pose na GPU: o modelo fica na GPU e a CPU só manda as matrizes das partes.
Os modelos Bedrock montam e enviam todos os vértices na CPU (animação em passos e vértices
guardados já cortaram parte disso; o que sobra é estrutural).

## Como o caminho vanilla funciona

- `mobs/skin.rs` (`SkinModels`, `Skin`): cada `Arc<Part>` é achatado uma vez em `SkinVertex`
  (posição/16, UV, normal como eixo, índice da parte) e guardado (limite de 4 M vértices). Por
  quadro, `Skin::draw(model, base, poses, Paint)` caminha as partes, calcula as matrizes e empurra
  `SkinJob`s (uma corrida de vértices com uma pintura: retângulo de textura, `scroll`, cor, luz,
  overlay, glint, modo, `cull`).
- `gfx/entity_skin.rs` + `entity_skin.wgsl`: uma compute escreve os mesmos `Vertex` da CPU no
  buffer de entidades. Layout do buffer: `[posados na GPU][19 materiais da CPU][contornos posados]`.
  A faixa posada é desenhada junto com o material 0 (entidades opacas e recortadas comuns).
- Critério do modelo: toda normal de polígono é um eixo exato (±x, ±y, ±z). A caminhada respeita
  `poses.skip_draw` (ossos escondidos).
- Precedente de segundo destino: `SkinFrame::outline_jobs` posa as silhuetas das entidades com
  brilho numa faixa à parte, que o passe de contorno desenha.
- Testes: `entity_skin::expand` é a referência do shader em Rust; `posed_on_the_gpu_every_mob_draws_what_the_cpu_draws`
  compara bit a bit GPU e CPU; um teste roda o shader de verdade contra `expand`.

## Respostas da análise

### 1. Os modelos Bedrock passam no critério?

Sim. `Geometry::to_tree` faz de um cubo rotacionado uma parte filha no pivô dele, então os
polígonos ficam alinhados aos eixos. Na medição nenhum modelo foi barrado pelo critério.
`bake` já produz `Arc<Part>` estáveis por geometria (`BedrockMobs::built`).

Tamanho: o A&S tem 3715 geometrias e 56 417 cubos; todas guardadas seriam ~1,35 M vértices
(~36 MB), abaixo do limite. Só as desenhadas são guardadas.

### 2. O que impede cada camada (medido por vértice desenhado)

| Mob | Já cabe (`PLAIN`) | Resto |
|---|---|---|
| Vaca | 100% | — |
| Creeper | 98% | 2% camada de cima |
| Slime | 67% | 33% camada de cima (cubo externo) |
| Aldeão | 32% | 68% camadas de cima |
| Aranha | ~0% | 99% camada de cima, 1% UV animado |
| Golem de cobre | ~0% | 98,5% stencil, 1,5% UV animado |

O que a CPU faz depois de desenhar uma camada (`render_03.rs`) e como fica na GPU:

| Retoque na CPU | Na GPU |
|---|---|
| Viés de profundidade das camadas de cima (`lift_toward_camera`: posição menos `push` na direção da câmera) | Campo no job (o `spare` de hoje) aplicado no shader: as posições já são relativas à câmera |
| Recorte de faces traseiras (`cull_back_faces`) | `cull` por job (hoje é por modelo): o shader de entidades já descarta faces traseiras com `SHADED_CULL_W` |
| Luz achatada (`flatten_lighting`: normal `[0,127,0,0]`) | Um modo novo do job |
| Marcação de material (`tag_bedrock_material`: OIT, aditivo, sem escrita/teste de profundidade) | Exige faixas posadas por material (ver 3) |
| Brilho emissivo (segunda passada, luz cheia, viés, `mark_oit`) | Também exige faixa posada OIT |
| Stencil (`hand_over`) | Faixa posada para o passe de stencil (ver 4) |
| UV animado com volta (`animate_uv`: deslocamento + escala, `rem_euclid`) | Não cabe nos 16 campos do job; fica na CPU (raro: 1–1,5%) |
| Inversão de culling, casco invertido por stencil (`stencil_not_equal` sem o passe) | Ficam na CPU (raros) |

### 3. Como misturar CPU e GPU num mesmo mob

O renderizador separa os pipelines por faixa: os quads da CPU são ordenados em 19 materiais pelo
campo `layer`, e a faixa posada entra no material 0. Os bits de material que o shader escreve no
`layer` são ignorados ali. Consequências:

- Camadas opacas e recortadas podem ir para a GPU e as outras continuar na CPU no mesmo mob: as
  camadas com blend, aditivas ou sem profundidade já são desenhadas em materiais posteriores, e
  entre as opacas manda o teste de profundidade (com o viés das camadas de cima indo junto).
- Levar camadas translúcidas ou aditivas para a GPU exigiria uma faixa posada por material, com o
  `EntityBatch` contando-as. Fica para depois: medir antes se vale.

### 4. Stencil (o golem de cobre)

Hoje `bedrock_stencil::hand_over` empurra cópias dos vértices da CPU para o passe de stencil, que
tem buffer próprio e desenha lotes em ordem (`draw_indexed` com `base + first`). Proposta, no
padrão de `outline_jobs`:

- `SkinFrame` ganha `stencil_jobs` e `stencil_vertices`; o shader escreve essas cópias numa faixa
  depois dos contornos no buffer de entidades.
- Os lotes do passe de stencil ganham a origem do vértice (buffer próprio ou faixa posada); o passe
  liga o buffer de entidades para os lotes posados. A compute roda antes dos passes de entidades.
- `Route::SceneAndPass`: um job para a cena e um para o stencil. `Route::Pass`: só o do stencil.
- Mão em primeira pessoa (`Target::Hand`) fica na CPU, como toda a primeira pessoa.
- Ponto a verificar ao implementar: `prune` e `order` reordenam os lotes; a origem tem que ir junto.

## Decisões

1. Reaproveitar `Skin`/`SkinFrame`/`entity_skin` (nada de um caminho paralelo): os modelos Bedrock
   entram por `Skin::draw`, grupo de material a grupo de material, com a CPU como reserva por grupo.
2. A pose (Molang) continua na CPU, com os passos por distância e por multidão; só a geometria vai
   para a GPU.
3. Fora do caminho de GPU: primeira pessoa, prévias da interface, cubo de enxofre, UV animado,
   inversão de culling, casco invertido sem o passe, camadas translúcidas/aditivas (até a etapa 5).
4. Vértices guardados (`KeptMesh`): um mob que usou a GPU guarda a parte da CPU e também o que
   posou na GPU (`KeptPose`: matrizes das partes, jobs da cena e do stencil, peças entregues ao passe,
   limites). Enquanto a pose não muda, as matrizes são movidas pelo deslocamento do mob em vez de
   caminhar o modelo de novo; vale só na mesma época do armazenamento de modelos (`SkinModels::epoch`).
5. Sem GPU de compute (ou `HORIZON_NO_ENTITY_SKIN`), tudo segue na CPU como hoje.

## Ordem de implementação

| Etapa | O que | Libera | Teste |
|---|---|---|---|
| 1 | Passar `Skin` ao `render_bedrock`; grupos `PLAIN` por `Skin::draw` | Vacas, creepers, corpo dos slimes, parte dos aldeões | Bedrock pela GPU = CPU bit a bit (via `expand`) num pack de teste e no A&S; jogo: cena e fazenda |
| 2 | Viés de profundidade, `cull` por job e luz achatada no shader | Aranhas, aldeões, slimes inteiros | Os mesmos testes, com camadas de cima |
| 3 | Stencil posado (`stencil_jobs`, lotes com origem) | Golems de cobre e o que usa stencil | Os lotes posados desenham os mesmos vértices que os da CPU; olhar o golem no jogo |
| 4 | Medir de novo na fazenda e na cena; decidir a etapa 5 | — | Socket de controle |
| 5 (talvez) | Faixas posadas por material (OIT, aditivo, sem profundidade), brilho emissivo | Camadas translúcidas e brilhos | — |

Critério de sucesso na fazenda de golems: `eg_mobs` e `renderer` perto dos números sem add-on.

## Riscos

- Diferenças sutis de imagem entre CPU e GPU (arredondamento de UV, `inset_uv`, normais): os
  testes bit a bit pegam; o caminho vanilla já resolveu os mesmos casos.
- Packs de shader: o caminho de GPU tem um ramo próprio (`dispatch_pack`); o passe de stencil fica
  desligado sob packs de shader, então a etapa 3 não os afeta.
- Ordem entre camadas coplanares: hoje resolvida pelo viés na CPU; na GPU, pelo viés no shader
  (etapa 2). Até lá, camadas de cima ficam na CPU.
- Contornos de mobs com brilho: `outline_since` já cobre os jobs posados.
