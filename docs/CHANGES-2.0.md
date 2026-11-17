# Horizon Bedrock 2.0: desempenho do motor de add-ons

Mudanças desde a `bedrock-v275`. Versão focada em estabilidade de memória e desempenho com
add-ons Bedrock pesados (testada com Actions & Stuff 1.13). Os números vêm de um servidor com
uma fazenda de golems de cobre (~90 golems em cena), build release.

## Resultados

| Cena | Antes (1.x) | 2.0 |
|---|---|---|
| Fazenda de golems (servidor) | 66–110 FPS | 130–180 FPS |
| Lugar sem mobs, espada na mão (CPU por quadro) | 2,16 ms | GPU faz a mão e o item |
| Montagem dos mobs (`bd_mesh` + `bd_kept`) | ~2,0 ms | ~0,5 ms |
| Preparo do passe de stencil | 0,49 ms | 0,06 ms |
| Pose Molang de um golem (host real do jogo) | 114 µs | 99 µs |

## Memória e travamentos

- Sprites com profundidade de pixel (`use_pixel_depth`): os sprites de 1024 px do A&S geravam
  72 milhões de caixas (~15 GB) ao carregar os mobs, e o jogo era morto por falta de memória antes
  da tela inicial. Agora a silhueta tem no máximo 32 células por lado, e cada modelo e textura é
  lido uma vez só.
- Terreno distante (LOD): um bloco que continuava ativo ficava com amostras antigas na malha
  (pânico em `check_mesh_samples`); agora a malha usa as amostras novas.
- Registro de memória: uma thread anota no log a memória residente e o heap quando mudam 256 MB,
  avisa ao passar de 3/4 do limite e mostra quem ocupa o heap (build `alloc-census`). A recarga de
  packs, a conversão Bedrock e o carregamento dos mobs registram tempo, memória e pico.

## Primeira pessoa

- As duas mãos passam por um único view model Bedrock; as queries de uso e de comer seguem a mão
  ativa; as animações recomeçam ao voltar para a primeira pessoa.

## Desempenho na CPU (antes da GPU)

- Menos alocações por quadro no motor Bedrock (pose e montagem), e um benchmark de pose
  (`anim_bench`).
- Mobs distantes têm a pose recalculada menos vezes, com fases espalhadas entre os quadros; em
  multidões grandes, os passos começam mais perto.
- Pose mantida: um mob cuja pose não mudou é desenhado de novo a partir dos vértices que já
  montou, movidos junto com ele.

## Modelos dos packs na GPU

- Mobs dos packs posados na GPU, como os mobs vanilla: camadas comuns, viés das camadas de
  cima, peças do passe de stencil (golem de cobre) e as matrizes das partes reaproveitadas
  entre quadros enquanto a pose não muda.
- Camada emissiva: a base vai para a GPU; o brilho (`_glow`) também, com a transparência
  melhorada desligada. Golem de cobre: de 160 para 16 vértices na CPU; enderman e aranha: 0.
- Mão em primeira pessoa (o jogador inteiro que o pack desenha para a mão) posada na GPU,
  escrita dentro dos vértices da própria mão (sem shader pack).
- Itens segurados e vestidos (attachables) posados na GPU, cada parte no osso do dono.
  Primeira pessoa com espada, picareta, escudo, arco: 0 vértices na CPU.
- Testes bit a bit: a GPU desenha exatamente o que a CPU desenhava (86 mobs do pack, com e sem
  stencil e transparência melhorada, com itens; primeira pessoa com vários itens).

## Molang e animação

- Interpretador ~30% mais rápido: fluxo de controle sem `Result` por nó, avaliação numérica
  direta, argumentos de queries na pilha, mapas por nome com FxHash.
- Consultas de osso (`q.bone_orientation_trs`, `q.bone_origin`) calculam só o osso pedido em vez
  do esqueleto inteiro; `Pose::get` sem alocar.
- Queries de item (`q.is_item_name_any`) sem cópias de texto em minúsculas.

## CPU do desenho

- Modelos de pixel extrudado (espadas e ferramentas 3D): faces internas entre pixels vizinhos
  removidas.
- Posição de repouso do osso do item e grupos de material guardados por modelo, em vez de
  recalculados a cada quadro.
- Passe de stencil: os vértices do quadro não são mais copiados para uma lista nova a cada quadro.

## Medição

- `bd_update` (tempo das atualizações de pose), `r_stencil_begin` e `r_stencil_draw` no trace.
- `HORIZON_BEDROCK_PROFILE=1`: tempo de pose por tipo de entidade no log, a cada 5 s.
- Testes ignorados para medir com o host real do jogo: `time_a_pack_mob_posed_with_the_game_host`
  (`HORIZON_POSE_KIND`, `HORIZON_POSE_HANDS`) e `what_a_pack_mob_leaves_on_the_cpu`.
- `anim_bench`: `ANIM_CHECK=1` (soma de verificação das poses) e `ANIM_ROWS`.

## Ferramentas

- Tarefas `bedrock-play` (compila e abre pelo launcher oficial), `bedrock-run` (abre o cliente
  direto com os arquivos preparados) e a tarefa completa de distribuição de teste Bedrock.
- `tools/ide_session.py`: perfis do IntelliJ usando a instância e a conta do launcher instalado,
  com limite de 8 GB de memória (`HORIZON_MEMORY_MAX`).
- README e status de validação do Bedrock atualizados.

## Organização do código

- `mobs/bedrock.rs` dividido: as respostas às queries foram para `mobs/bedrock_host.rs`.
- `mobs/render.rs` dividido: a escolha de texturas e camadas foi para `mobs/render_textures.rs`.

## Pendências conhecidas

Ver `docs/bedrock-pendencias.md`: golem de cobre piscando com o stencil, brilhos e slimes com
shader pack, borda vermelha do arco encantado, animação de pulo, idle, elytra com textura de capa,
som da lança, variante do lobo. Próximo passo de desempenho: compilar o Molang para bytecode.
