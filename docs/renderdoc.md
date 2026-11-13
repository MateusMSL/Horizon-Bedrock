# RenderDoc: comparar Bedrock e Horizon desenho a desenho

Para defeitos de stencil, profundidade e ordem de desenho (relógio do A&S, halo da lula
brilhante, brilho de armadura): em vez de deduzir pelo resultado, capturar um quadro e ver cada
desenho com o estado da placa de vídeo.

## Instalar

1. Baixar em <https://renderdoc.org> (Windows, 64 bits) e instalar.
2. Abrir o **RenderDoc** (`qrenderdoc.exe`).

## Capturar o Bedrock

1. Fechar o Minecraft Bedrock.
2. RenderDoc ➜ aba **Launch Application**:
   - **Executable Path**: `C:\XboxGames\Minecraft for Windows\Content\Minecraft.Windows.exe`
   - **Working Directory**: `C:\XboxGames\Minecraft for Windows\Content`
   - **Capture Options**: marcar *Ref All Resources* e *Capture Callstacks* (opcional).
3. **Launch**. Um texto do RenderDoc aparece no canto do jogo ("Direct3D 12 ... F12 to capture").
   Se o texto não aparecer ou o jogo recusar abrir, o Bedrock bloqueia a injeção: avisar (o
   plano B são as variantes geradas do pack de teste).
4. Entrar no mundo só com o Actions & Stuff, segurar o relógio em 1ª pessoa, parado.
5. **F12** (ou *Capture Frame(s) Immediately* no RenderDoc). Fechar o jogo.
6. Na lista de capturas do RenderDoc, salvar como `bedrock-relogio.rdc`.

## Capturar o Horizon

O mesmo, com:
- **Executable Path**: `C:\Users\Mateus\Documents\GitHub\Horizon-Bedrock\target\fast\horizon-client.exe`
- **Working Directory**: a pasta do repositório (a mesma da configuração "Horizon - Bedrock debug"
  do IntelliJ, com os mesmos argumentos em **Command-line Arguments**).
- O Horizon usa Vulkan: a captura funciona sem configurar nada.
- Salvar como `horizon-relogio.rdc`.

## O que olhar (ou me mandar)

Mandar os dois `.rdc` (podem ser grandes: compactar) ou, se preferir, prints destas telas:

1. **Event Browser** com a lista de desenhos perto do item na mão (no Horizon os passes têm
   nome: `First-person hand`, `Bedrock stencil (hand)`, `Bedrock stencil (world)`).
2. Clicando no desenho do mostrador: **Pipeline State ➜ OM** (Output Merger): Depth Test,
   Depth Write, Stencil (função, referência, operações, máscaras).
3. **Texture Viewer** com o alvo de profundidade/stencil selecionado, canal **Stencil**
   (botão de canais), no mesmo desenho: mostra o valor do stencil em cada pixel.
4. Clicando com o botão direito num pixel do mostrador que deveria aparecer ➜ **History**
   (Pixel History): diz qual desenho escreveu o pixel e por qual teste cada um falhou
   (profundidade ou stencil).

Com 1–4 do Bedrock dá para saber em que ordem ele desenha caixa, janela e mostrador e por que o
mostrador passa; com os mesmos do Horizon, onde o Horizon diverge.
