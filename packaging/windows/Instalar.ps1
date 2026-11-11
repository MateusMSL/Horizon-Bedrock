$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$exe = Join-Path $root 'horizon-client.exe'
$base = Join-Path $env:APPDATA 'horizon-launcher'
$vers = Join-Path $base 'versions'
if (-not (Test-Path $exe)) { Write-Host 'horizon-client.exe ausente ao lado do script.'; exit 1 }
if (-not (Test-Path $vers)) { Write-Host 'Horizon nao encontrado. Instalar o launcher em playhorizon.dev e preparar o 26.3.'; exit 1 }
$sel = $null
$cfg = Join-Path $base 'settings.json'
if (Test-Path $cfg) { $sel = (Get-Content $cfg -Raw | ConvertFrom-Json).selected }
$dir = $null
if ($sel -and (Test-Path (Join-Path $vers "$sel\.local\horizon-client\26.3"))) { $dir = Join-Path $vers $sel }
else {
  $dir = Get-ChildItem $vers -Directory | Sort-Object Name -Descending |
    Where-Object { Test-Path (Join-Path $_.FullName '.local\horizon-client\26.3') } |
    Select-Object -First 1 -ExpandProperty FullName
}
if (-not $dir) { Write-Host 'Nenhum nightly preparado. Abrir o launcher e dar Play uma vez.'; exit 1 }
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { Write-Host 'Fechar o jogo antes.'; exit 1 }
$target = Join-Path $dir 'horizon-client.exe'
$orig = "$target.orig"
if (-not (Test-Path $orig)) { Copy-Item $target $orig }
Copy-Item $exe $target -Force
$store = Join-Path $base 'addons'
New-Item -ItemType Directory -Force $store | Out-Null
$link = Join-Path $dir 'addons'
if (-not (Test-Path $link)) { New-Item -ItemType Junction -Path $link -Target $store | Out-Null }
Write-Host "Instalado em $dir"
Write-Host "Add-ons: $store"
