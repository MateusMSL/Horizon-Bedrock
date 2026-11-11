# Horizon Bedrock: compilar e instalar

O build do Bedrock entra no launcher oficial do Horizon como uma versão importada. Conta, versões e configurações do launcher continuam valendo.

## Requisitos

- Launcher oficial do Horizon instalado, com um nightly instalado e preparado (aba Versões).
- Minecraft Java 26.3 aberto uma vez no launcher oficial da Mojang.
- Rust (rustup) e Git.
- Add-on Bedrock (`.mcpack` ou pasta), por exemplo o Actions & Stuff.

## 1. Limpar e compilar

```powershell
cd C:\Users\Mateus\Documents\GitHub\Horizon-Bedrock
```

Limpeza só do cliente (rápida):

```powershell
cargo clean -p horizon-client
```

Limpeza total (várias GB, recompila tudo):

```powershell
cargo clean
```

Compilar:

```powershell
cargo build --release -p horizon-client
```

Resultado: `target\release\horizon-client.exe`.

Conferir os hooks após um `git rebase upstream/main`:

```powershell
.\tools\check-hooks.ps1
cargo check -p horizon-client
```

## 2. Adicionar o exe ao Horizon

1. Fechar o jogo e o launcher. Fechar processos presos:

   ```powershell
   Get-Process horizon-client -ErrorAction SilentlyContinue | Stop-Process -Force
   ```

2. Abrir o launcher do Horizon, aba **Versões**.
3. Clicar em **Importar uma compilação do jogo…**.
4. Escolher `C:\Users\Mateus\Documents\GitHub\Horizon-Bedrock\target\release\horizon-client.exe`.
5. Base: um nightly instalado e preparado. Confirmar o aviso de build sem assinatura.
6. A versão **Imported · data** fica selecionada. Clicar em **Jogar**.

Atualizar o build: compilar de novo e importar de novo. Cada importação cria uma versão nova; remover as antigas na aba Versões.

Erro "Acesso negado" ao importar: fechar o jogo, mover a pasta `.horizon-cache` do nightly base para fora da pasta `versions` e importar de novo. O cache é recriado.

## 3. Adicionar o add-on (mcpack)

1. Copiar o `.mcpack` (ou a pasta do add-on) para a pasta `addons` da versão importada:

   ```
   %APPDATA%\horizon-launcher\versions\imported-<data>-<sha>\addons
   ```

2. Abrir o jogo, **Opções > Addons**, ativar o add-on.
3. A primeira abertura converte o pack (cerca de 1 minuto). As seguintes usam cache.

## 4. Configurações recomendadas

No menu de configurações do Actions & Stuff (settings do add-on, dentro do jogo):

- Ativar itens em 3D.
- Ativar arrows (flechas).
- Desativar expressions (expressões).
- Modo **Full Experience**.

No Horizon:

- Opções > Addons: add-on ativo, acima dos pacotes de recursos.
- Gráficos em Vulkan, como no padrão do launcher.

## Diagnóstico

- Log do jogo: `%APPDATA%\horizon-launcher\versions\imported-<data>-<sha>\logs\latest.log`.
- Hooks do Bedrock nos arquivos originais do Horizon: [bedrock-hooks.md](bedrock-hooks.md).
- Estado da engine: [bedrock-roadmap.md](bedrock-roadmap.md).
