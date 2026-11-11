# Bedrock add-ons: estado com o A&S 1.1.68

Pack de teste: Actions & Stuff (Oreville). Motor genérico, sem ajuste por pack.

## Cobertura

| Medida | Valor |
| --- | --- |
| Leitura do pack (4483 JSON) | 100% (0 falhas) |
| Molang (199430 expressões) | 100% interpretadas (0 falhas) |
| Chaves ignoradas (`audit_pack`) | 0 |
| Client entities validadas em jogo | 18 de 128 (14%) |
| Client entities com erro conhecido | 20 de 128 (16%) |
| Client entities sem erro conhecido, ainda não conferidas | 90 de 128 (70%) |

Validadas: allay, vex, vindicator, papagaio, enderman, shulker, abelha, cavalo (3), burro (3), coelho (2), axolote, golem de cobre, creaking.

## Erros identificados

Mobs
- Slime: casca menos translúcida que no Bedrock
- Vaca cogumelo: parede vermelha gigante junto à câmera
- Cubo de enxofre: exibição quebrada
- Boneco de neve: sem variação de cabeça (abóbora)
- Lhama de mercador: sem tapete
- Devastador: efeito de stun fora da cabeça
- Blaze: sem animação de ataque
- Creeper carregado: fumaça diferente do Bedrock
- Ghast / happy ghast: sem animação de ataque, sem bola de fogo da textura, brilho do arnês
- Baiacu: sem forma inflada
- Breeze: tornado some
- Phantom: fumaça colada
- Pillager: besta mal posicionada
- Tatu: cabeça piscando
- Aldeão fazendeiro: chapéu
- Cavalo esqueleto: sela
- Zumbi husky e abelha: olhos/piscar
- Armor stand: armaduras vanilla em vez das do pack; animação no chão ausente

Brilho
- Olhos de aranha, enderman, esqueleto
- Arreio do ghast, tocha
- Glint de armadura (onda colorida), contorno encantado

Motor
- `is_local_player`, `is_spectator`, `is_emoting`, `hurt_direction`, `property` (parcial), estados de mob (sleeping, shaking, digging etc.)
- Variáveis `v.*` do jogo não definidas (elytra, glow_alpha, is_enchanted)
- Entidades fora do caminho do pack: armor stand, minecart, barco, projéteis
- Item no inventário, hotbar, GUI e no chão
- 1ª pessoa: braço do pack, ataque
- Ataque duplicado na 3ª pessoa (espada, tridente)
- Texturas de item/bloco sem equivalente Java
- Subpacks: `scan_pack` com subpack
- Fogs e biomas de cliente: lidos, não desenhados
- Behavior packs: fora de escopo atual
