# Actions & Stuff: checklist de validação

Mapa do conteúdo do pack segundo o guia oficial (https://guides.orevillestudios.com/actions-and-stuff, versão 1.13), nas categorias do site. Cada item é um teste no Horizon contra o Bedrock (fonte de verdade).

Legenda: `[x]` validado no jogo · `[~]` parcial / com bug conhecido · `[ ]` não validado.
Ao marcar, anotar a versão da engine (`ENGINE_BUILD`) entre parênteses. Bugs encontrados vão para `docs/bedrock-pendencias.md`.

---

## 1. General

### 1.1 Animações do jogador (1ª e 3ª pessoa)
- [ ] Movimento: correr, andar direcional, pular, aterrissar, nadar
  - Obs.: voando, a armadura desconecta do corpo
- [ ] Idles (poses paradas)
  - Obs.: algumas existem; falta verificar todas
- [x] Ferramentas: machado, picareta, pá, enxada
- [~] Armas: espada (ataque 1ª/3ª validado v267), arco, maça, tridente, besta, lança (lança faz som de estocada só segurando)
- [x] Utilidades: luneta, pincel
- [~] Diversos: comer, beber, subir escada
  - Obs.: sem animação de subir escada
- [x] Postura de combate (sacar espada entra na postura e volta após cada golpe)
- [ ] Pesca: lançar e recolher com a vara 3D
  - Obs.: idle com textura do A&S existe; sem animação de lançar, bug visual (imagem no Notion)
- [ ] Mapa: desenrolar e segurar nivelado com as duas mãos
  - Obs.: 1ª pessoa sem mapa diferente e sem animação; 3ª pessoa tem animação
- [x] Montaria: inclinar nas curvas + balanço de 1ª pessoa (cavalo, camelo, lhama, porco, strider)
- [x] Movimentos especiais (pack setting "Special Movement Animations"): barrel roll de elytra, super hero landing, mergulho/bomba na água, water bucket clutch
- [ ] Mortes do jogador: queda, fogo, água; outros jogadores caem de lado ao morrer por golpe lateral
  - Obs.: sem animações de morte, nem jogador nem mobs
- [ ] Ataque acelera com Haste e desacelera com Mining Fatigue
  - Obs.: sem variação
- [x] Capa anima montado e fica por fora do peitoral
- [ ] Capa presa sob a arma nas costas (holster), balança lateral
  - Obs.: mecânica de holster inexistente

### 1.2 Animações de mobs
- [x] Movimento: mover, pular, aterrissar, nadar
- [x] Rostos expressivos: rastreio de olhos, sobrancelhas
- [x] Ataque e dano (com variações)
- [~] Efeitos melhorados: creeper carregado, wither
  - Obs.: precisa de melhorias
- [x] Armadura, ferramentas e armas 3D em mobs (zumbi, esqueleto etc.)
- [x] Idles
- [x] Inclinação de montaria: cavalo, burro, mula, cavalo esqueleto/zumbi, porco, strider, happy ghast (cavaleiro inclina junto)
- [~] Lobos uivam em noite de lua cheia
  - Obs.: uivam independente da lua cheia
- [ ] Iron Golem idle de entregar papoula

### 1.3 Animações de morte (80+ mobs)
Teste: `/effect @s strength infinite 255 true`; queda: `/summon cow ~~50~5`.
- [ ] Reage à direção do golpe (frente, costas, lados) com variações
- [ ] Morte na água (fundo: flutua/deriva; superfície: cai e afunda; corpo afunda um pouco)
- [ ] Morte na lava (afunda)
- [ ] Bebês com mortes próprias
- [ ] Queda: impacto no chão (vanilla flutua alto)
- [ ] Esqueleto, Stray, Bogged, Wither Skeleton: desmontam em ossos (wither mais pesado)
- [ ] Cavalo esqueleto desmonta (variantes laterais)
- [ ] Blaze e Breeze: peças com física; na água flutuam/afundam
- [ ] Snow Golem derrete em poça, cabeça e galhos voam
- [ ] Bruxa sugada pelo chapéu ou perde o chapéu
- [ ] Warden: almas saindo do peito
- [ ] Ender Dragon dissolve para dentro com flashes roxos + "End Ash" caindo
- [ ] Enderman desbota para branco com fumaça
- [ ] Tatu rola para longe do golpe (mesmo enrolado)
- [ ] Abelha gira para longe do golpe
- [ ] Morcego, papagaio, allay, vex: sobem e afundam devagar no ar
- [ ] Ghast e Happy Ghast: gritam/ficam tristes, balançam, recuam
- [ ] Golfinho: deita de lado/costas em terra e fecha os olhos
- [ ] Lula e lula brilhante: se contorcem fora d'água
- [ ] Sapo: rola ou mostra a língua; afoga de barriga para cima
- [ ] Aranha e aranha da caverna: se enrolam, viram de costas por golpe frontal
- [ ] Zumbi, Husk, Drowned: caem para trás, desabam, cambaleiam, caem de lado
- [ ] Aldeão, mercador, bruxa, zumbi aldeão, illagers, família piglin: caem de lado
- [ ] Lobo: pisca, mantém humor/cauda, morre na pose (sentado, deitado, dormindo)
- [ ] Coelho: vira, senta ou desaba
- [ ] Strider: afunda na lava, desaba na água, cai em terra
- [ ] Hoglin e Zoglin morrem onde estão
- [ ] Slime, Magma Cube, Sulfur Cube: grandes se dividem; pequenos caem
- [ ] Shulker pisca, gira cabeça/tampa, encara e estoura

### 1.4 Partículas e efeitos de morte
- [ ] Partículas gerais (explosões, brilhos, poeira, respingos)
- [ ] Poof de morte novo (versões água, lava, Nether)
- [ ] Efeitos únicos: Warden, Ender Dragon, Enderman, Evoker, Allay, Vex, Blaze, Witch, Wither Skeleton, Copper Golem
- [ ] Bolhas que brilham na morte subaquática
- [ ] Som do poof (mais grave em mob grande, ignição extra no Nether)

### 1.5 Ambiência
- [ ] Névoa e cores de céu customizadas
- [ ] Dappled Forest: céu, névoa e atmosfera próprios
- [ ] Partículas de ambiente: inventariar e testar uma a uma (o guia não lista). Para cada efeito no Bedrock: onde aparece, gatilho, duração/frequência; comparar com o Horizon. Só validar após a lista completa.

### 1.6 Sons
- [x] Golpes de espada novos
- [x] Balde de axolote como companheiro
- [x] Passos sincronizados com a animação

### 1.7 Armadura 3D
- [x] Couro
- [x] Malha
- [x] Cobre
- [x] Ferro
- [x] Ouro
- [x] Diamante
- [~] Netherite (contorno/brilho do encantamento ajustado v268/v269, aguardando validação)
- [x] Casco de tartaruga (anima nadando, "ganha vida")
- [x] Armour Shimmer em diamante, ferro, ouro, cobre não encantados (pack setting "Armour Shimmer")
- [x] Armadura 3D quando segurada na mão
- [ ] Armadura visível em 1ª pessoa (Vanilla Experience)

### 1.8 Encantamento (contorno e glint custom)
- [x] Espada 2D: contorno (v255+)
- [x] Cor do glint pelo pack setting (v258/v259)
- [~] Intensidade do contorno/pulso vs luz do mundo (v268/v269)
- [x] Armadura vestida
- [~] Armadura segurada
  - Obs.: aparece, sem encantamento
- [ ] Elytra
- [x] Escudo (também com banner)
- [x] Carrot on a Stick e Warped Fungus on a Stick
- [ ] Bússola de magnetita
- [~] Bússola de recuperação: não aponta o local da morte, gira infinitamente
- [x] Livro escrito
- [~] Maçã dourada encantada: textura bugada em 1ª pessoa
- [ ] Cabeças de mob (Curse of Binding/Vanishing)
  - Obs.: texturas corretas; 1ª pessoa difícil de enxergar, verificar
- [ ] Itens no holster mostram glint e contorno

### 1.9 Holstered Weapons
Pack setting "Holstered Weapons": Back Rear, Back Front, Belt Left, Belt Right (3 de 4 ativos por padrão). Só no próprio jogador, Full e Vanilla Experience.
- [ ] Espada, machado, picareta, pá, enxada, maça: costas ou cinto
  - Obs.: nenhum exibido
- [ ] Arco, besta, lança, tridente: costas
  - Obs.: nenhum exibido
- [ ] Luneta: cinto
  - Obs.: não exibida
- [ ] Mostra o melhor item de cada tipo; vai para a mão ao trocar de slot
- [ ] Oculto nas costas com elytra
- [ ] Desmarcar todos desliga

### 1.10 Suporte de armadura
- [~] 12 poses (Sneak + Usar item no suporte)
  - Obs.: só coloca o item; poses não mudam
- [ ] Desempenho

### 1.11 Camas customizadas
- [~] Camas 3D
  - Obs.: aparência vanilla; textura/modelo do pack não aplicados

### 1.12 Itens 3D
Detalhe por item em `docs/bedrock-items-3d.md`.
- [ ] Todo item segurado é 3D
  - Validar por grupos (modelo, textura, orientação, escala; 1ª e 3ª pessoa; duas mãos): ferramentas; armas (espada, maça, lança, tridente); arco e besta (puxados/carregados); utilidades (luneta, pincel, bússola, mapa, vara); combate/uso (escudo, totem, pérola e olho do Ender, wind charge); consumíveis (comidas, poções, baldes); equipamentos (armaduras, elytra, armadura de lobo, arreio de Happy Ghast). Verificar animação ao segurar, usar, atacar, carregar, lançar, recolher.
- [ ] Vara de pesca 3D
  - Obs.: verificar modelo e partes móveis; lançar e recolher
- [x] Armadura 3D na mão
- [x] Elytra 3D na mão
- [x] Escudo 3D na mão
- [ ] Armadura de lobo 3D
  - Obs.: todas as variantes; aparência no lobo equipado
- [~] Arreio de Happy Ghast 3D
  - Obs.: na mão aparece a textura que vai sobre o Happy Ghast em vez do modelo do item (imagem no Notion)
- [ ] Itens com animação/partícula própria
  - Por item e evento, confirmando no Bedrock: vara (lançar/recolher), arco e besta, tridente e lança, escudo (bloqueio), pérola e olho do Ender, totem, cristal do End, estrela do Nether, wind charge, itens encantados; props com efeito (fogueiras, tocha/braseiro, roda d'água, moinho, espantalho). Registrar efeito, gatilho, duração, frequência, som; segurado vs usado vs prop.

### 1.13 Retrabalhos de mobs
- [x] Bruxa (modelo e textura novos)
- [ ] Strider (modelo e textura novos, Full Experience)

### 1.14 Diversos
- [~] Flores de vitória-régia menos frequentes
  - Obs.: flores não aparecem; sem variação da textura base
- [ ] Textura do atril
- [~] Partículas de plantio vanilla-style na Vanilla Experience
  - Obs.: semente jogada no chão sem animação/efeito

---

## 2. Nametags

Status: PULADO por enquanto (2.1 a 2.6).

### 2.1 General
- [ ] Qualquer mob (menos Ender Dragon): `Baby` mantém filhote
- [ ] Allay: `Goldie` (dourado; também natural)
- [ ] Abelha: `Smol` (bebê); `Qween`, `Crown` (coroa; 0,5% natural); `Queen`
- [ ] Camel Husk: `Calf`, `Camelo`
- [ ] Gato: `Oiia Oiia` (gira para sempre); `Party`, `Purrty` (chapéu de bolo)
- [ ] Galinha: `Chick`/`Baby`; `Chicken Jockey`; `Cooked`/`Nugget`; `Lava Chicken`; `Duck` (verde ou branco, grasna); `Rooster` (canta); `Turkey` (gorgoleja); `Chickle`/`Sea Chickle`
- [ ] Creeper: `Pig`; `Creeperling`/`Boomlet` (bebê)
- [ ] Golfinho: `Bubblegum` (rosa)
- [ ] Enderman: `Beanie`, `Green` (olhos verdes)
- [ ] Raposa: `Raccoon`; `Jimothy`
- [ ] Ghast: `Boo` (ghast bebê amigável)
- [ ] Happy Ghast: `Creeper Balloon`, `Enderman Balloon`, `Pig Balloon`, `Pumpkin Balloon`; `Saddleless`
- [ ] Husk: `Mummy`, `Dusty` (faraó)
- [ ] Iron Golem: `Agent` (terno)
- [ ] Nautilus: `Saddleless`; `Armourless`; `Nautiling`/`Whorl`/`Baby`
- [ ] Panda: `Grizzly` (marrom; preto raro; Sneezy vira Honey; sons de urso polar); `Freddy` (cartola)
- [ ] Papagaio: `Crow`, `Robin`, `Blue Jay`, `Cardinal`, `Turtle Dove`
- [ ] Porco: `Muddy`, `Mudster`, `Mucky` (também natural raro)
- [ ] Slime: `jeb_`, `RBG`, `Disco` (RGB)
- [ ] Aldeão: `Beekeeper`; `Hoppity` (Páscoa); `Villager32` (glitched); `Hazmat` (sons próprios); `Replica`/`Life Form` (3 variantes aleatórias) e `Replica Tall`/`Stretched`, `Replica Twins`/`Twin`, `Replica Triplets`/`Triplet` (também invertido: `Tall Replica`)
- [ ] Warden: `William`; `Bacteria` (sons, sonic charge/boom próprios, tentáculos animados)
- [ ] Bruxa: `Emeralda` (verde; 5% natural)
- [ ] Lobo: `Party`, `Pawty` (chapéu de aniversário)
- [ ] Zumbi: `Brian`
- [ ] Zombie (Coral) Nautilus: `Nautiling`, `Zombilus`, `Baby`

### 2.2 Halloween
Pack setting "Seasonal Mob Variants" faz spawn natural o ano todo; ou nametag `Halloween` (exceto Trick-or-Treaters, porco e galinha esqueleto). Nome de bebê depois da barra.
- [ ] Allay: `Ghost`, `Wisp`
- [ ] Tatu: `Pumpkin` / `Pumpkin Pup`
- [ ] Axolote: `Gremlin` / `Baby Gremlin` (balde de axolote com Gremlin com o setting ligado; funciona com "Axolotl Colour")
- [ ] Morcego: `Vampire`, `Batula`
- [ ] Camelo: `Mummy` / `Mummy Calf`
- [ ] Camel Husk: `Executioner` / `Executioner Calf`
- [ ] Gato e jaguatirica: `Witch` / `Witch Kitten`, `Baby Witch`
- [ ] Galinha: `Zombified` / `Baby Zombified`; `Skeleton` / `Baby Skeleton`
- [ ] Copper Golem: `Voodoo Doll`, `Voodoo`, `Doll`
- [ ] Vaca e Mooshroom: `Zombified` / `Zombified Calf`
- [ ] Creaking: `Twiggy`, `Creakling`, `Baby`
- [ ] Creeper: `Skeleton`
- [ ] Drowned: `Gill Man`, `Gillman` / `Baby Gill Man`
- [ ] Enderman: `Magician`
- [ ] Endermite: `Eye`
- [ ] Evoker: `Dracula`
- [ ] Sapo: `Imp`, `Gremlin`, `Gargoyle`
- [ ] Cabra: `Evil Goat` / `Evil Kiddo`, `Baby Evil Goat`
- [ ] Cavalo: `Wendigo` / `Wendigo Foal`
- [ ] Husk e Parched: `Executioner` / `Baby Executioner`
- [ ] Iron Golem: `Frank`, `Frankenstein`
- [ ] Nautilus e Zombie Nautilus: `Sea Monster` / `Sea Monster Nautiling`, `Baby Sea Monster`
- [ ] Papagaio: `Crow`
- [ ] Phantom: `Bat`
- [ ] Porco: `Zombified` / `Zombified Piglet`; `Skeleton` / `Baby Skeleton`
- [ ] Piglin, Piglin Brute, Zombified Piglin: `Butcher` / `Baby Butcher`
- [ ] Pillager: `Helsing` (itens aleatórios no cinto, poções brilhantes)
- [ ] Coelho: `Killer Bunny`
- [ ] Ovelha: `Ghost` / `Ghost Lamb` (tosquiar revela esqueleto)
- [ ] Shulker: `Pumpkin` (projéteis viram abóboras brilhantes com o setting)
- [ ] Traça: `Centipede` (onda de perninhas)
- [ ] Esqueleto e Bogged: `Scarecrow`
- [ ] Slime: `Skull`
- [ ] Aranha e aranha da caverna: `Skull`
- [ ] Stray: `Grim Reaper`, `Reaper` (lanterna brilhante balançando)
- [ ] Strider: `Demon` / `Demon Stridling`
- [ ] Tartaruga: `Nessie`
- [ ] Aldeão: `Ghost`, `Boo` (desempregado e nitwit); `Trick or Treat`, `Trick or Treater` (fantasia por profissão)
- [ ] Vindicator: `Clown`, `Giggles` (passos, dano e risada próprios)
- [ ] Mercador e lhama do mercador: `Fortune Teller`
- [ ] Warden: `Jack`, `Gourden` (morte própria)
- [ ] Bruxa: `Cackle`, `Warty` (5% natural)
- [ ] Lobo: `Hellhound`, `Hell Hound` / `Hellhound Pup`, `Baby Hellhound`; `Skeleton`, `Marrow` (com bebê)
- [ ] Zumbi: `Frankenstein` / `Baby Frankenstein`

### 2.3 Festive
- [ ] Allay: `Elf`, `Jingle`
- [ ] Gato: `Elf`, `Jingle`
- [ ] Copper Golem: `Robot`, `Windup`
- [ ] Creaking: `Spruce`, `Twinkle` (árvore de Natal)
- [ ] Enderman: `Nutcracker` (tambores somem segurando bloco)
- [ ] Cavalo: `Rudolph`, `Reindeer` (sela especial domado)
- [ ] Iron Golem: `Claus` (dá presente a aldeão bebê; no pool de idle)
- [ ] Pillager: `Krampus`
- [ ] Ravager: `Krampus`
- [ ] Slime: `Snowman` (0,05% natural em bioma frio sem cartola; nametag 50% com cartola)
- [ ] Snow Golem: `Snowman`, `Frosty`
- [ ] Aldeão: `Elf`
- [ ] Warden: `Yeti`
- [ ] Lobo: `Reindeer`, `Reindog` (sinos na coleira domado)
- [ ] Zumbi: `Zanta`; `Snowman` (mesma regra do slime)

### 2.4 Snowy (spawn natural em bioma frio)
- [ ] Galinha (variante fria, chance de neve)
- [ ] Vaca (variante fria, chance de neve)
- [ ] Creeper (+ `Creeperling`/`Baby` em natural = bebê nevado)
- [ ] Enderman (+ `Endie`/`Baby` em natural = bebê nevado)
- [ ] Drowned e Drowned bebê
- [ ] Cavalo
- [ ] Iron Golem
- [ ] Porco (variante fria, chance de neve)
- [ ] Pillager
- [ ] Ovelha
- [ ] Esqueleto
- [ ] Slime
- [ ] Aranha
- [ ] Lhama do mercador
- [ ] Aldeão
- [ ] Mercador
- [ ] Bruxa
- [ ] Zumbi e zumbi bebê
- [ ] Zumbi aldeão e bebê

### 2.5 Dappled Forest
Spawn natural no bioma ou nametag `Dappled`; pack setting "Entity Biome Variants" desliga. Hostis com fungos.
- [ ] Galinha
- [ ] Vaca
- [ ] Creaking
- [ ] Creeper
- [ ] Enderman
- [ ] Iron Golem
- [ ] Porco
- [ ] Ovelha
- [ ] Esqueleto
- [ ] Sniffer
- [ ] Aranha
- [ ] Aldeão
- [ ] Zumbi

### 2.6 Pale Garden
Spawn natural no bioma ou nametag `Pale`; "Entity Biome Variants" desliga. Olhos brilhantes.
- [ ] Creeper
- [ ] Enderman (terceiro olho)
- [ ] Esqueleto
- [ ] Aranha
- [ ] Zumbi

---

## 3. Props

Status: PULADO por enquanto; retomar pendentes depois.

Suporte de armadura renomeado `§§` ou `Prop` (`/summon armor_stand §§`). Variantes a 45°: `/execute as @n[type=armor_stand] at @s run tp @s ~~~ 45`.
- [x] Posicionamento do prop no suporte (v253)

### 3.1 Static Props
- [ ] Farinha de osso: restos de esqueleto
- [ ] Tijolos: chaminé (fumaça)
- [ ] Bolo: bolo de festa
- [ ] Cenoura: pilha de cenouras
- [ ] Pedregulho: aglomerado de pedras
- [ ] Biscoito: pote de biscoitos
- [ ] Cabeça de creeper: estátua de anatomia
- [ ] Pena: forte de travesseiros
- [ ] Caco Heartbreak: cerâmica quebrada
- [ ] Balde de lava: lava lamp
- [ ] Barco de mangue: trenó festivo
- [ ] Tapete de musgo: cobertura musgosa
- [ ] Pedregulho musgoso: aglomerado musgoso
- [ ] Espeleotema pontiagudo: gárgula
- [ ] Abóbora: canteiro de abóboras
- [ ] Vela vermelha: candelabro
- [ ] Tocha de redstone: pilha de blocos de redstone
- [ ] Crânio de esqueleto: crânio com vela
- [ ] Bloco de neve: boneco de neve
- [ ] Neve: cobertura de neve
- [ ] Lanterna da alma: lâmpada gótica
- [ ] Tocha da alma: pilar de fogo de pedra
- [ ] Alçapão de abeto: roda d'água (respingos na água, sons)
- [ ] Tijolos de pedra: pilha de tijolos
- [ ] Tocha: braseiro medieval (partículas)
- [ ] Balde d'água: banheira de pássaros
- [ ] Escadas de lã: tendas (todas as cores)

### 3.2 Interactive Props (animam ao socar)
- [ ] Cogumelo marrom: 3D
- [ ] Fogueira: 4 estados (acesa, apagada, cinzas, sem fogo), partículas e sons
- [ ] Carrot on a Stick: pinhata Strider
- [ ] Mesa de cartografia: globo
- [ ] Tijolos de pedra cinzelados: lápide de zumbi (ganha vida à meia-noite)
- [ ] Sementes de cacau: casa de gengibre (3 batidas na porta)
- [ ] Fungo carmesim: 3D
- [ ] Pá de diamante: monte de neve com pá
- [ ] Olho de Ender: retrato de Enderman (olhos seguem, jump scare)
- [ ] Fornalha: lareira (chamas, sons)
- [ ] Moldura brilhante: entrada dos Backrooms (fita azul, porta)
- [ ] Jack o'Lantern: abóbora grande (tampa pula)
- [ ] Leaf Litter: folhas com ancinho (puff)
- [ ] Couro: varal de curtume (chão/parede)
- [ ] Porta de mangue: caixão de vampiro (acorda)
- [ ] Alçapão de pale oak: moinho (pás grande/pequena, velocidade, orientação/hélice, sons)
- [ ] Balde de neve fofa: globo de neve grande
- [ ] Trilho: trem de brinquedo
- [ ] Cogumelo vermelho: 3D
- [ ] Bola de neve: globo de neve pequeno
- [ ] Fogueira da alma: partículas e sons
- [ ] Barco de abeto: carroça (poses; blocos dentro)
- [ ] Cerca de abeto: suporte de armas (tamanhos)
- [ ] Folhas de abeto: guirlanda
- [ ] Muda de abeto: árvore de Natal
- [ ] Graveto: pilha de lenha (poses)
- [ ] Açúcar: balde de doces de abóbora
- [ ] Bagas doces: azevinho em vaso
- [ ] Baú armadilha: Jack in the Box (jump scare)
- [ ] Twisting Vines: bengala verde (45° = grande)
- [ ] Fungo distorcido: 3D
- [ ] Warped Fungus on a Stick: pinhata Lhama
- [ ] Weeping Vines: bengala vermelha (45° = grande)
- [ ] Trigo: espantalho (ganha vida à noite, 2 idles, partículas)
- [ ] Vela branca: vela festiva
- [ ] Botas de couro: meias de Natal
- [ ] Bundles: presentes (animação e sons)
- [ ] Lãs: pelúcias

### 3.3 Cycling Props (Agachar + Interagir troca variante)
- [ ] Tapetes: toldo / tapete
- [ ] Baiacu: balões de festa

### 3.4 Placeable Items

Status: PULADO por enquanto.
Suporte renomeado `§§§§` ou `Item`.
- [x] Posicionamento (v253)
- [ ] Qualquer item (menos armadura)
- [ ] Poses: em pé, encostado, deitado, de lado, pendurado (interagir troca)
- [ ] Animações na 1ª pose: pérola do Ender, olho do Ender, totem, cristal do End, estrela do Nether, wind charge, estátua de Copper Golem

### 3.5 Cushions
3 animações de sentar que amassam a almofada. Versão invisível: `§` antes de cada letra (ex.: `§s§o§f§a`).
- [~] Sentar (3 animações aleatórias)
  - Obs.: senta, mas sempre a mesma animação
- [ ] Ground Cover: `Moss`, `Snow`, `Sand`, `Leaves`, `Cold Leaves`, `Pale Leaves`, `Orange Leaves`, `Warm Leaves`, `Pale Moss`, `Nether Wart`, `Warped Wart` (com ou sem "Cover")
- [ ] Misc: `Straw`, `Shingles`/`Brick Tiles`, `Whoopee`/`Whoopee Cushion` (som de pum), `Quilted` (padrão por cor)
- [ ] Sofa/Couch: normal, `Left`, `Right`, `Middle`, `Middle Legs`, `Corner`; `Cowch`
- [ ] `Chair`, `Stool` (carvalho por padrão)
- [ ] Bush: `Bush`, `Cold`, `Orange`, `Pale`, `Warm`, flores `Blue`, `Orange`, `Pink`, `Purple`, `Red`, `White`, `Yellow`
- [ ] `Lamp`/`Light` (brilha, toda cor de lã)
- [ ] `Planter Box`, `Planter Pot`, `Basket`
- [ ] `Display Case`/`Case`, `Wide`, `Tall`
- [ ] `Planks`
- [ ] `Block` (textura do bloco)
- [ ] `Log Stool`
- [ ] Cogumelo: `Mushroom Cap`/`Mushroom`, `Red`, `Brown`, `Shelf`, `Crimson`, `Warped`
- [ ] Prato: `Ceramic Plate`, `Ceramic Decorative Plate`, `Wood Plate`, `Blue`/`Green`/`Red`/`Yellow Plastic Plate` (`Dark Plate`)
- [ ] Tigela: mesmas variantes de prato com `Bowl`
- [ ] `Invisible`

---

## 4. Morphs
- [ ] Disabled (modelo padrão)
- [ ] Mini Me (jogador bebê)
- [ ] Rana (garota sapo)
- [ ] Pixie (Allay)
- [ ] Lucy (Axolote)
- [ ] Ash (cavaleiro Blaze)
- [ ] Iris (olho do Ender)
- [ ] Bandit (raposa)
- [ ] Rusty (golem de redstone)
- [ ] Echo (Sculk Whisperer)
- [ ] Goober (Slime)
- [ ] Sully (Sulfur Cube)
- [ ] Russell (Bushling: folhas mudam de cor por bioma, variante por madeira, sons)

---

## 5. Costumes
Couro + acabamento de armadura (trim) vira a fantasia. Também em mobs via dispensador.
- [ ] Rana Puffercoat: Shaper
- [ ] Elf Attire: Wild
- [ ] Santa Getup: Snout
- [ ] Viking Armour: Rib
- [ ] Party Set: Host
- [ ] Hazmat Suit: Silence
- [ ] Fantasia em mob (zumbi via dispensador)

---

## 6. Hats
Cosmético na cabeça: `/replaceitem entity @s slot.armor.head 0 <item>`.
- [ ] `cactus`
- [ ] `cake`
- [ ] `hay_block`
- [ ] `emerald`
- [ ] `lit_pumpkin`
- [ ] `carved_pumpkin`

---

## 7. Expressions
Skins com expressões (skin pack ou template próprio).
- [ ] Piscar
- [ ] Movimento dos olhos
- [ ] Sobrancelhas animadas
- [ ] Tipos de olho e de sobrancelha do template

---

## 8. Secrets
- [ ] Logo Oreville (Cole) no slot de capacete da UI
- [ ] Creeper carregado mais eletrificado
- [ ] Casco de tartaruga ganha vida e nada junto
- [ ] Abóboras com variações de rosto
- [ ] Abóbora pálida (3% de chance)
- [ ] Easter eggs de nametag: `Goldie`, `Pig`, `Beanie`/`Green`, `Muddy`, `jeb_`, `Villager32`, `William`, `Brian`, `Chickle` (ver 2.1)
- [ ] Almofada `Whoopee`

---

## 9. Texture Styles (subpacks)
- [ ] Full Experience (tudo)
- [ ] Vanilla Experience (animações e 3D com texturas vanilla)
- [ ] Custom Experience (animações e 3D, compatível com outros texture packs acima)

---

## 10. Pack Settings citados no guia
- [ ] Special Movement Animations
- [ ] Armour Shimmer
- [ ] Holstered Weapons (4 slots)
- [ ] Seasonal Mob Variants
- [ ] Entity Biome Variants
- [ ] Axolotl Colour
- [x] Cor do glint do encantamento (v258)
- [ ] Subpack (Texture Styles)
