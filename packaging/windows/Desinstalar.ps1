$ErrorActionPreference = 'Stop'
$vers = Join-Path $env:APPDATA 'horizon-launcher\versions'
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { Write-Host 'Fechar o jogo antes.'; exit 1 }
$n = 0
foreach ($d in Get-ChildItem $vers -Directory) {
  $t = Join-Path $d.FullName 'horizon-client.exe'
  if (Test-Path "$t.orig") { Move-Item "$t.orig" $t -Force; $n++; Write-Host "Restaurado $($d.Name)" }
}
if (-not $n) { Write-Host 'Nada para restaurar.' }
