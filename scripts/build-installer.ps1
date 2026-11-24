# Builds the release executable and the Windows installer.
#
#   powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1 -SkipBuild
#
# The installer lands in dist/. Inno Setup 6 must be installed for the current user,
# which is the default when installing from https://jrsoftware.org/isdl.php.

param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $root
try {
    if (-not $SkipBuild) {
        Write-Output "building release binary"
        cargo build --release -p multideck-desktop
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }

    $iscc = Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"
    if (-not (Test-Path $iscc)) {
        throw "Inno Setup 6 was not found at $iscc. Install it from https://jrsoftware.org/isdl.php"
    }

    Write-Output "compiling installer"
    & $iscc "packaging\windows\multideck.iss"
    if ($LASTEXITCODE -ne 0) {
        throw "ISCC failed with exit code $LASTEXITCODE"
    }

    Get-ChildItem (Join-Path $root "dist") -Filter *.exe |
        ForEach-Object { Write-Output "installer: $($_.FullName)" }
}
finally {
    Pop-Location
}
