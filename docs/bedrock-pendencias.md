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
- [ ] Tesoura do A&S mostrando os braços.
- [ ] Vara: escala 0 em 1ª pessoa no pack de teste.
- [ ] Ataque em 3ª pessoa tocando duas animações por clique.
- [ ] Tridente com pose invertida.
- [?] Relógio: mostrador (v220), hora (v221). Bússola apontando (v216).
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

- [?] Desempenho com o stencil na GPU (v215; comparar com `cpu_stencil`).
- [ ] `texture_meshes`: tamanho real e extrusão.
- [ ] `uv_anim` por material/osso.
- [ ] `filter_lighting`, `rebuild_animation_matrices`.
- [ ] Variáveis de partícula por partícula.
- [ ] `entity_size` no cliente.
- [ ] Log de campos ignorados e queries sem resposta.
- [ ] Ferramentas de debug sem recompilar (em andamento por partes no `bedrock-debug.json`).
- [ ] Migrar o add-on para a base atual do Horizon com ganchos mínimos.
