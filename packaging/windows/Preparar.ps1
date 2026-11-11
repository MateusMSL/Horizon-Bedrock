$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$mc = if ($env:HORIZON_MINECRAFT_DIR) { $env:HORIZON_MINECRAFT_DIR } else { Join-Path $env:APPDATA '.minecraft' }
$meta = Join-Path $mc 'versions\26.3\26.3.json'
if (-not (Test-Path $meta)) {
  Write-Host "Minecraft Java 26.3 nao encontrado em $mc."
  Write-Host "Abrir o launcher oficial, jogar 26.3 uma vez e rodar de novo."
  exit 1
}
if (-not (Get-Command java -ErrorAction SilentlyContinue)) {
  Write-Host "Java 25 nao encontrado. Instalar: https://adoptium.net"
  exit 1
}
$jar = Join-Path $PSScriptRoot 'server.jar'
if (-not (Test-Path $jar)) {
  $url = (Get-Content $meta -Raw | ConvertFrom-Json).downloads.server.url
  Write-Host "Baixando server.jar da Mojang..."
  Invoke-WebRequest $url -OutFile $jar
}
Write-Host "Preparando arquivos do jogo (uma vez)..."
& .\horizon-prepare.exe --prepare --server-jar $jar --minecraft-dir $mc
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& .\horizon-prepare.exe sounds --minecraft-dir $mc
exit 0
