$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$bad = 0
foreach ($l in Get-Content tools/bedrock-hooks.txt -Encoding UTF8) {
    if (-not $l) { continue }
    $i = $l.IndexOf('|'); $f = $l.Substring(0, $i); $s = $l.Substring($i + 1)
    if (-not (Test-Path $f) -or -not (Select-String -Path $f -SimpleMatch -Quiet -Pattern $s)) {
        Write-Host "FALTA  $f  ::  $s" -ForegroundColor Red; $bad++
    }
}
if ($bad) { Write-Host "$bad hooks ausentes. Reaplicar: git apply --3way patches/bedrock-hooks.patch"; exit 1 }
Write-Host 'hooks ok'
