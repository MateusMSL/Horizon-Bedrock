# Build do fork para o launcher

O jogo do fork é distribuído como uma build que o launcher oficial importa (aba **Versões > Importar uma compilação do jogo…**). Não vai script junto: quem abre o jogo é o launcher, com a conta que está logada nele.

## Gerar

```
cargo run -p xtask -- bedrock-build
```

No IntelliJ: configuração **Horizon - Bedrock release**.

A tarefa (`xtask/src/bedrock_build.rs`) compila o `horizon-official` com o perfil `release`, igual ao workflow de release, e copia o programa para `release/horizon-bedrock/horizon-client.exe`.

- `horizon-official` é o jogo com as partes do Horizon (amigos, presença, servidores oficiais, Discord). O binário `horizon-client` das configurações dev não tem essas partes.
- O launcher aceita um programa solto só com nome começando em `horizon-client`, ou um `.zip`/`.tar.gz` que tenha `horizon-client.exe` dentro. Por isso, distribuir `release/horizon-bedrock.zip` com a pasta `release/horizon-bedrock`.

## Conta

Nada a configurar. Antes de abrir qualquer versão, importada ou não, o launcher confere se a conta é dona do jogo e entrega a sessão ao programa (`--session-stdin`/`--session-file`).

## Arquivos do jogo

A versão importada é uma cópia de um Nightly instalado com o programa trocado. O `research-lock.json` do fork tem que ser igual ao do Nightly base: os recursos preparados dele servem.

## Add-ons

O jogo lê `addons/` da pasta da versão importada. Packs de terceiros (Actions & Stuff) não vão no zip; o `LEIA-ME.md` explica onde colocar.
