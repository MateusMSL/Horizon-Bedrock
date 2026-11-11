# Ajuste do item na mão (primeira pessoa)

Arquivo: `bedrock-debug.json` (pasta do jogo). Salvar recarrega em 1 s. Vale só para o item do pack na mão.

```json
{
    "debug": true,
    "fp_offset": [-0.12, -0.12, 0.0],
    "fp_rotate": [0.0, 0.0, -6.0],
    "fp_scale": 0.8
}
```

| Chave | Valores | Unidade |
|---|---|---|
| `fp_offset` | `[lateral, altura, distância]` | blocos |
| `fp_rotate` | `[x, y, z]` | graus |
| `fp_scale` | número | 1 = tamanho do modelo |

Padrão atual: offset `[-0.12, -0.12, 0.0]`, rotate `[0, 0, -6]`, escala `0.8`.

## Valores por item

Preencher a partir do que ficar certo no jogo.

| Item | offset lateral | offset altura | offset distância | rot x | rot y | rot z | escala |
|---|---|---|---|---|---|---|---|
| espada | | | | | | | |
| machado | | | | | | | |
| picareta | | | | | | | |
| pá | | | | | | | |
| enxada | | | | | | | |
| lança | | | | | | | |
| tridente | | | | | | | |
| arco | | | | | | | |
| besta | | | | | | | |
| escudo | | | | | | | |
| balde | | | | | | | |
| poção | | | | | | | |
| bloco | | | | | | | |
| tocha | | | | | | | |
| outro: | | | | | | | |
