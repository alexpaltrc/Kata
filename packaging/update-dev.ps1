<#
  Brings Kata Dev up to date: brings in what was done on GitHub (from the cloud or elsewhere),
  builds Kata Dev, installs it over the one in use and opens it again. Stable Kata is not touched.

    powershell -ExecutionPolicy Bypass -File packaging\update-dev.ps1 [-NoPull]
#>
param([switch]$NoPull)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

Push-Location $repo
try {
    if (-not $NoPull) {
        Write-Host "Bajando los cambios de GitHub..."
        git pull --ff-only
        if ($LASTEXITCODE -ne 0) { throw "no se pudieron bajar los cambios de GitHub (git pull)" }
    }
    Write-Host "Fabricando e instalando Kata Dev..."
    & (Join-Path $PSScriptRoot "build-msix.ps1")

    $package = Get-AppxPackage -Name "AlexPalacios.Kata.Dev"
    Start-Process "explorer.exe" "shell:AppsFolder\$($package.PackageFamilyName)!App"
    Write-Host "Listo: Kata Dev $($package.Version) instalada y abierta." -ForegroundColor Green
} catch {
    Write-Host "No se pudo actualizar: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
} finally {
    Pop-Location
}
