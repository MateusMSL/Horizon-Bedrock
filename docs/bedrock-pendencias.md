# Pendências do add-on Bedrock (lista única)

Junta as listas pedidas na conversa (lista de tarefas da sessão, notas "Anotado" do
`bedrock-roadmap.md` e o relato de 2026-11-13). Detalhes por item em
`bedrock-itens-checklist.md`; histórico de versões em `bedrock-roadmap.md`.

`[~]` em andamento · `[ ]` aberto · `[?]` feito, falta confirmar no jogo.

## Itens na mão e attachables

- [~] Pederneira: animação de uso (botão direito segurado) quebrada, 1ª e 3ª pessoa (`use_duration`, `is_using_item`, controladores).
- [ ] Item de duas mãos com item na segunda mão: no Bedrock segura com uma mão só; no Horizon com as duas (captura de 2026-11-13: tocha-flor com totem na outra mão). O pack decide por `v.oreville_offhand_custom_pos`, `q.is_item_equipped('off_hand')` e `q.get_equipped_item_name(1)` (controladores `zyrcdq`, `wqjvtj`, `dpcxlx`); falta um dump em jogo dessas variáveis.
- [ ] Item na segunda mão balançando sem parar (`is_holding_left`, `attack_time` do dono, animação em loop).
- [ ] Vara de pesca: sem animação ao lançar (attachable `vyiidn`, controladores `jqlacc`/`ddvgjm`). [?] v222: visível de novo.
- [ ] Itens invisíveis ou com textura quebrada em 1ª, 3ª de costas e 3ª de frente; espada 2D; bola de vento na mão.
- [ ] Textura do pack sem modelo 3D/animação na mão: papel, mapas, teia de aranha, camas.
- [ ] Modelo 3D sem animação: trilho, discos, estande de armadura; capacete de tartaruga com animações parciais.
- [ ] Cristal do End sem 2D nem 3D.
- [ ] Poção só com a cor da água. Causa achada: o jogador do A&S lê o tipo da poção de `t.aux_data`, uma variável temporária que no Bedrock vaza do desenho do ícone no inventário (`render_controllers/pqr.json`); o Horizon não desenha attachables no inventário nem guarda temporárias entre scripts, e a vista do item não recebe os componentes da pilha. A ordem do array de texturas (`fzb.json`, 47) é o aux do Bedrock (0 água ... 46 infestação). Falta: levar o tipo da poção até o jogador e semear `v.ggcegi`/`t.aux_data`.
- [?] Partículas de itens (ametista e outros): v223 (partículas e sons de attachables saem); v224: uma por locator (não mais uma por quadro) e no lugar do item em 1ª pessoa. Pack de teste 1.0.10: fragmento de ametista.
- [?] Partículas presas a locators de mobs espelhadas atrás (faísca do creeper, sapo): v224 corrige `attach_point`.
- [ ] Pérola do ender piscando (5 partículas, 4 controladores; rever depois da v223).
- [ ] Vidros com textura errada na mão.
- [ ] Armadura vestida com pixels faltando (nariz do elmo).
- [x] Tesoura do A&S mostrando os braços (v267: animações do attachable fora dos ossos do dono).
- [ ] Vara: escala 0 em 1ª pessoa no pack de teste.
- [ ] Ataque em 3ª pessoa tocando duas animações por clique.
- [ ] Tridente com pose invertida.
- [?] Relógio: mostrador (v220), hora (v221). Bússola apontando (v216).
- [ ] Itens mal posicionados na mão em 1ª pessoa (cristal do End grande e deslocado; captura de 2026-11-13).
- [x] Tesoura em 1ª pessoa: braços gigantes cobrindo a tela (v267).
- [ ] Balde d'água: fundo/paredes internas transparentes (vê-se a grama). Material `entity_alphatest_one_sided`; paredes internas são um cubo de altura negativa (`3m4c6l`, `[6,-6.5,6]`); falta renderizar o attachable e conferir a face/culling.
- [ ] Trilho ativador, estande de armadura etc.: sem animação na primeira vez que são puxados (aquecimento de 6 s da v188 em attachable novo é suspeito; não alterado).
- [?] Ovo arremessado com a textura do ovo comum: o projétil `Egg` não guardava qual ovo era. Agora guarda o item (`ProjectileKind::Egg(item)`), desenha `item/blue_egg`/`item/brown_egg` e o pintinho nasce da variante do ovo (código base do Horizon, não do motor Bedrock; o pack desenha o ovo 3D só se o projétil virar entidade do pack).
- [ ] Armaduras do A&S (cobre) chapadas/rosadas conforme o ângulo da câmera, depois do stencil na GPU (v215). Contorno do pack: corpo escreve stencil 2 (`tasrgm`), casca inflada (`hyyvpv`/`zoytgm`) desenha onde `stencil & 18 == 0`. Onde a casca cobre a armadura o stencil 2 não foi escrito: suspeita do teste de profundidade da passada (cópia da profundidade da cena, `GreaterEqual`). Teste: `cpu_stencil: true` no `bedrock-debug.json`.
- [ ] Checklist de todas as animações e controladores do pack, marcando implementada ou não.

## Jogador

- [ ] Rosto preto da skin em 3ª pessoa.
- [ ] Capa, elytra e armaduras presas ao osso; depuração de posição de equipáveis.
- [ ] Nado, elytra, idle, fogo/lava.
- [ ] Diferença de iluminação Bedrock × Horizon (adiada pelo usuário).

## Mobs

- [ ] Creeper carregado: pulso, rastro, rosto branco, contorno.
- [ ] Contorno de allay e vex; halo fraco da aranha; arnês do ghast.
- [ ] Glow de arreios de ghast, de tocha, olhos de aranha/enderman/esqueleto (ferramenta de escala/intensidade por camada).
- [ ] Olho do allay; husk piscando; olho da abelha; baiacu inflando; filhotes (aldeão, vaca, geral).
- [ ] Slime, cubo de enxofre, magma: textura; cubo de enxofre tremendo, parando a animação e com o item absorvido desalinhado.
- [ ] Estande de armadura: armaduras do pack e animação colocado no chão.
- [ ] Golem de cobre.
- [ ] Golem de cobre piscando e com o olho bugado depois da pose na GPU (branch `bedrock-gpu-pose`, relato de 2026-11-17; os outros mobs ok). Capturas: num quadro o corpo sai chapado, cor de cobre sem textura e sem a camada de cima; no outro sai normal. Suspeitas: o stencil posado (`hand_over_posed`, o corpo escreve o stencil que a casca lê) reaproveitado do quadro anterior (`KeptPose`: peças do passe com o lugar errado no buffer posado) ou a ordem entre os lotes posados e os da CPU no passe de stencil. Com shader pack (stencil desligado) não pisca. Testar com `HORIZON_NO_ENTITY_SKIN=1` e com `cpu_stencil: true` no `bedrock-debug.json` para separar GPU, stencil e reaproveitamento.
- [?] Lula brilhante: halo atravessando parede (v214).
- [?] Ovelha "Halloween" (v219, confirmado).

## Mundo, partículas e ambiente

- [?] Pingos de chuva no chão, respingo de água, crítico, coração, aldeão feliz, fumaça de fogueira, chama de vela, folhas do álamo: v223 troca a partícula do jogo pelo efeito do pack (`bedrock-particulas.md`). Pack de teste 1.0.10: chuva magenta, respingo amarelo, crítico vermelho.
- [?] Respingo ao cair na água: coroa baixa (v232, velocidade da entrada na água). Gotas altas, anéis e onda já iguais ao Bedrock.
- [ ] Riscos da chuva: o A&S traz `textures/environment/weather.png` (32×32, chuva e neve lado a lado); o conversor só lê `environment/rain` e `snow`, então a chuva fica a do Java (azul). Falta separar o `weather.png` em `rain.png`/`snow.png`.
- [ ] Partículas do pack sem equivalente Java (portal direcional, explosões do dragão, `mobspell_emitter`...) e as que o pack faz como fluxo (totem, gás, bolhas): listadas em `bedrock-particulas.md`. `atmospherics`, `fogs`, `lighting`: a revisar.
- [?] Fogo aceso sem a textura do pack: `fire_0`/`fire_1` do A&S são tiras de 32 quadros sem `flipbook`; v223 escreve o `.png.mcmeta` de animação (conversor 24).
- [ ] Texturas de bloco do pack em tira animadas pelo `flipbook_textures.json` vanilla do Bedrock (prismarina, portal, algas, grama marinha, sculk, minérios do A&S...): o pack não traz o `flipbook` delas e o conversor não escreve o `.png.mcmeta`; conferir no jogo se aparecem espremidas.
- [ ] Modelos do pack em item no chão, moldura, GUI e blocos colocados (alavanca, minecart, barco).

## Motor

- [ ] Otimizar o motor: FPS bem abaixo do Horizon base (anotado em 2026-11-14). Medir com e sem pack (`--benchmark`, `HORIZON_SPIKES`) e achar onde o motor Bedrock gasta: Molang/controladores por mob e por quadro, attachables, partículas, stencil.
- [ ] Medidores de desempenho do motor Bedrock (anotado em 2026-11-15): tempo por quadro de Molang/controladores, animações, attachables, partículas, stencil e desenho, por mob e no total; contagem de mobs e vértices do pack. Visível no F3 ou num relatório (`--benchmark`), para comparar Horizon base × com add-on e medir cada otimização.
- [ ] Otimizações a partir dos medidores: o Horizon base roda bem, com o A&S cai bastante (motor novo ainda sem otimização).
- [?] Desempenho com o stencil na GPU (v215; comparar com `cpu_stencil`).
- [ ] `texture_meshes`: tamanho real e extrusão.
- [ ] `uv_anim` por material/osso.
- [ ] `filter_lighting`, `rebuild_animation_matrices`.
- [ ] Variáveis de partícula por partícula.
- [ ] `entity_size` no cliente.
- [ ] Log de campos ignorados e queries sem resposta.
- [ ] Ferramentas de debug sem recompilar (em andamento por partes no `bedrock-debug.json`).
- [ ] Migrar o add-on para a base atual do Horizon com ganchos mínimos.
- [ ] Lança na mão toca o som de estocada sozinha (sem atacar), A&S.

## Do checklist A&S (`bedrock-as-checklist.md`, anotado no Notion 2026-11-15)

- [x] Voando (elytra): armadura desconecta do corpo (v271, validado).
- [x] Brilho pulsante da armadura sempre verde (v272) e glint da elytra fora da asa (v273), validados.
- [ ] Sem animação de subir escada.
- [ ] Vara de pesca: sem animação de lançar/recolher; bug visual ao lançar.
- [ ] Mapa em 1ª pessoa: sem modelo e sem animação (3ª pessoa ok).
- [ ] Animações de morte: nenhuma, jogador e mobs (seção 1.3 inteira), poof e efeitos de morte (1.4).
- [ ] Velocidade do ataque com Haste / Mining Fatigue.
- [ ] Holstered Weapons: nada aparece nas costas/cinto.
- [ ] Creeper carregado e wither: efeitos a melhorar.
- [ ] Lobos uivam fora da lua cheia.
- [ ] Lobo com a skin (variante) errada: a textura não segue a variante do lobo (capturas de 2026-11-16: um lobo com textura escura e marrom misturada a peças cinza-metálicas, como se fosse a armadura ou outra variante; outro escuro de coleira vermelha). Ver a escolha de textura do render controller do lobo no A&S (`q.variant`/`q.mark_variant` e o array de texturas) e o que o Horizon responde para a variante (pálido, bosque, cinzento, preto, castanho, ruivo, malhado, listrado, nevado).
- [ ] Armadura segurada sem encantamento.
- [ ] Bússola de recuperação gira infinitamente, não aponta a morte.
- [ ] Maçã dourada encantada: textura bugada em 1ª pessoa.
- [x] Suporte de armadura: poses trocam com agachar + usar (v274, validado).
- [ ] Camas 3D do pack não aplicadas.
- [ ] Arreio de Happy Ghast na mão mostra a textura do ghast em vez do item.
- [ ] Vitória-régia: flores e variação de textura ausentes.
- [ ] Semente jogada sem animação/partícula de plantio.
- [ ] Almofada: sempre a mesma animação de sentar (3 aleatórias).
- [ ] Props interativos não reagem ao soco mesmo com a v275 (`hurt_direction` alternando por golpe). Investigar o que o pack lê no golpe.

## Do checklist A&S, Props, Morphs, Costumes e Secrets (Notion 2026-11-15)

- [ ] Chamas, brilhos e partículas de props desalinhados/acima do lugar (candelabro, bolo, crânio com vela, lanterna da alma, gárgula, cogumelos e fungos, fogueiras, lápide). Provável causa comum com o offset do Placeable Item.
- [ ] Placeable Item (`Item`/`§§§§`) voltou a deslocar: ~3 blocos acima e 2 à esquerda do pé do suporte (era v253).
- [ ] Props não aplicados: chaminé (tijolos), canteiro de abóboras, boneco de neve (bloco de neve), pilar de fogo (tocha da alma), entrada dos Backrooms (moldura brilhante), abóbora grande (Jack o'Lantern), melancia e fatia, carrinho com baú.
- [ ] Neve gera boneco de neve em vez da cobertura de neve.
- [ ] Escudo em suporte `Prop` esconde o corpo do suporte (no Bedrock o suporte segue normal).
- [ ] Props de soco não reagem: pinhatas Strider e Lhama, cogumelos/fungos, casa de gengibre (só com Shift). Validar com v275 no sobrevivência.
- [ ] Roda d'água: Shift + clique não troca pose/animação.
- [ ] Espantalho ganha vida de dia (condição de horário).
- [ ] Lápide: o zumbi que surge está morrendo.
- [ ] Braseiro (tocha) apagado e sem partículas.
- [ ] Lava lamp opaca (sem translucidez).
- [ ] Biscoito atravessa o vidro do pote.
- [ ] Monte de neve com pá: neve ausente e pá dentro da terra; globo de neve pequeno dentro da terra e incompleto.
- [ ] Meias de Natal: botas tratadas como bota vestida do suporte em vez de item na mão.
- [ ] Pelúcia da lã cinza-clara ausente.
- [ ] Morphs: em 1ª pessoa a mão vazia mostra a mão da skin original (com item aparece a do Morph).
- [ ] Morphs: ferramentas e armaduras não encaixam no corpo do Morph (testado em raposa, olho do fim e golem de redstone).
- [ ] Costumes: trim aparece na armadura de couro, mas nenhuma fantasia (opção ligada).
- [ ] Abóboras e Snow Golem sempre com o mesmo rosto (sem variações).
- [ ] Compatibilidade com shader packs (o A&S não suporta shaders oficialmente; relato de 2026-11-17, erros anteriores à otimização de desempenho):
  - Glows bugados: olhos brilhantes, tochas, allays e outros brilhos estouram num borrão branco (olhos do golem de cobre, allay inteiro). Revisar a passagem emissiva com shaders.
  - Slimes chapados ("flat"): a camada externa translúcida sai opaca e sem sombreamento, escondendo o corpo de dentro.
  - O golem de cobre não pisca com shaders: o passe de stencil fica desligado sob shader packs, o que reforça a suspeita no stencil posado (item do golem em Mobs).
- [ ] Texture Style Vanilla Experience com outro resource pack por cima: itens bugam.
- [ ] Nametags: avaliar uma a uma; `Halloween` no slime não aplica a variante de caveira.

## Relato de 2026-11-17 (analisado na madrugada, sem jogo do usuário aberto)

- [ ] Arco encantado com borda vermelha, qualquer que seja a cor de brilho escolhida. Causa provável: o render controller do contorno encantado do arco (`controller.render.oreville_ans.zxhwjj`, em `render_controllers/hcy.json`) tem `overlay_color` vermelho fixo (`r=1, g=0, b=0, a=1`), e o Horizon mistura `overlay_color` em toda camada (`Layer::tint`, `crates/horizon-bedrock/src/entity.rs`). A cor escolhida vem da textura (`Array.ncyhyp[v.nwypnn]`). Os materiais da camada (`oreville_njlxva`, `oreville_vrwhkp`, `oreville_wcjlza`) herdam de `entity_emissive`. Falta confirmar em que materiais o Bedrock aplica `overlay_color` (o define `USE_OVERLAY` do shader) antes de mudar: aplicar só onde o shader lê, sem quebrar o flash de dano dos mobs.
- [?] Mobs sem animação de morte. No mundo de teste (um jogador, servidor integrado) a morte do A&S roda inteira: o zumbi tomba em ~16 ticks (ragdoll em Molang: `v.eewgjn`, `q.bone_origin`), solta as partículas e some na fumaça; o motor offline faz o mesmo. O cliente trata o evento 3 (`EntityEvent.DEATH`) como o vanilla (vida 0). Se no servidor não aparece, a suspeita é o servidor: plugins que empilham mobs removem o mob na hora da morte. Falta ver num mob morto no servidor (sem mexer na fazenda).
- [ ] Animação de pulo cortada no meio. No mundo de teste o controlador `ngeixo` (`controller.animation.oreville_ans.lfjeov`) entra em `jump` ao sair do chão e volta a `default` quando `q.vertical_speed < -7` (regra do pack), um tick antes do pouso; a animação (`mykcsi`) tem 1,17 s e é misturada para fora em 0,2 s. Com a física do Bedrock a regra corta no mesmo ponto, então falta ver no jogo do usuário se o corte é outro (velocidade vertical do jogador local, `q.is_jumping`, pulo correndo `ypqtmb`/`seridw`).
- [ ] Faltam animações de idle (suspeita do usuário). Não investigado.
- [ ] Elytra com textura de capa sem o brilho de encantamento. Não investigado.
- [ ] Lança faz o som de ataque repetidamente só de estar na mão (já anotado em Motor). Os sons de golpe do A&S são de `sounds.json` (`item.*_spear.attack_miss` etc.); falta achar o que os dispara sem ataque.

Ferramentas novas para essas investigações: `HORIZON_BEDROCK_PROFILE=1` (tempo de pose por entidade no log) e o teste ignorado `time_a_pack_mob_posed_with_the_game_host` (`HORIZON_POSE_KIND`, mede e confere a pose com o host do jogo).
