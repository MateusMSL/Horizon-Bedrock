# Horizon Bedrock

Versão do Horizon que lê add-ons do Minecraft Bedrock (`.mcpack`): mobs, itens, animações, sons e partículas.

Funciona dentro do launcher oficial do Horizon, com a sua conta e as suas configurações.

## O que você precisa

- Windows 64 bits.
- Uma conta Microsoft com o Minecraft: Java Edition.
- Minecraft Java **26.3** aberto uma vez no launcher da Mojang (para o Horizon preparar os arquivos).
- O add-on Bedrock que você quer usar (`.mcpack` ou pasta).

## Instalação

1. Baixar o launcher do Horizon em **playhorizon.dev** e instalar.
2. Abrir o launcher e entrar com a conta Microsoft.
3. Na aba **Versões**, instalar um **Nightly** e esperar a preparação terminar. Dar **Jogar** uma vez e fechar o jogo.
4. Extrair este zip em qualquer pasta.
5. Fechar o jogo, se estiver aberto.
6. No launcher, aba **Versões**, clicar em **Importar uma compilação do jogo…**.
7. Escolher o `horizon-client.exe` da pasta extraída.
8. Escolher o Nightly do passo 3 como base e confirmar o aviso de build sem assinatura.
9. A versão **Imported** fica selecionada.

## Adicionar o add-on

1. Abrir a pasta da versão importada:

   ```
   %APPDATA%\horizon-launcher\versions\
   ```

   Entrar na pasta que começa com `imported-`.

2. Colocar o `.mcpack` (ou a pasta do add-on) dentro de `addons`.
3. No launcher, clicar em **Jogar**.
4. No jogo: **Opções > Addons** e ativar o add-on.

A primeira abertura com um add-on novo leva cerca de 1 minuto: o pack é convertido uma vez. Depois abre rápido.

## Configurações recomendadas

No menu de configurações do add-on (Actions & Stuff e similares), dentro do jogo:

- Ativar itens em 3D.
- Ativar arrows (flechas).
- Desativar expressions (expressões).
- Modo **Full Experience**.

## Atualizar

Quando sair uma versão nova deste zip: extrair, importar de novo (passos 6 a 8) e apagar a versão `Imported` antiga na aba Versões. Os add-ons ficam na pasta `addons` da versão importada; copiar para a nova.

## Problemas

- **Acesso negado ao importar:** fechar o jogo e o launcher, abrir de novo e importar outra vez.
- **O jogo não abre:** o log fica em `%APPDATA%\horizon-launcher\versions\imported-…\logs\latest.log`.
- **Add-on não aparece:** conferir se está dentro de `addons` da versão **importada**, não da do Nightly.

## Avisos

- Build não oficial, sem assinatura. Não é afiliado nem apoiado pela Mojang, pela Microsoft ou pelos autores do Horizon.
- Horizon: Ludovic Marie e colaboradores (licença MIT). Engine Bedrock: Mateus.
- Add-ons Bedrock são de seus autores; este zip não inclui nenhum.
