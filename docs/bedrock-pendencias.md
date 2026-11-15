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
- [ ] Armadura segurada sem encantamento.
- [ ] Bússola de recuperação gira infinitamente, não aponta a morte.
- [ ] Maçã dourada encantada: textura bugada em 1ª pessoa.
- [?] Suporte de armadura: poses não mudam (Sneak + Usar). v274, aguardando validação.
- [ ] Camas 3D do pack não aplicadas.
- [ ] Arreio de Happy Ghast na mão mostra a textura do ghast em vez do item.
- [ ] Vitória-régia: flores e variação de textura ausentes.
- [ ] Semente jogada sem animação/partícula de plantio.
- [ ] Almofada: sempre a mesma animação de sentar (3 aleatórias).
- [?] Props interativos que animam ao socar (v275, `hurt_direction` alternando por golpe), aguardando validação.
