# Downloads the official Stream Deck desktop installer and extracts it locally.
#
#   powershell -ExecutionPolicy Bypass -File scripts/fetch-streamdeck.ps1
#
# Why: the vendor installer is a per-machine MSI that needs elevation. An
# administrative install (msiexec /a) unpacks the payload into a directory
# without registering anything, which is all static analysis needs. The extracted
# files stay under tmp/ and are gitignored.

param(
    [string]$Version = "7.6.1.23033",
    [string]$Destination = "tmp\streamdeck"
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$work = Join-Path $root $Destination
$msi = Join-Path $work "Stream_Deck_$Version.msi"
$extracted = Join-Path $work "extracted"

New-Item -ItemType Directory -Force -Path $work | Out-Null

if (-not (Test-Path $msi)) {
    $url = "https://edge.elgato.com/egc/windows/sd/Stream_Deck_$Version.msi"
    Write-Output "downloading $url"
    & curl.exe -L --fail --output $msi $url
    if ($LASTEXITCODE -ne 0) {
        throw "download failed with exit code $LASTEXITCODE"
    }
}

$hash = (Get-FileHash $msi -Algorithm SHA256).Hash
Write-Output "sha256 $hash"

if (-not (Test-Path (Join-Path $extracted "PFiles64"))) {
    Write-Output "extracting with an administrative install (no system changes)"
    New-Item -ItemType Directory -Force -Path $extracted | Out-Null
    $log = Join-Path $work "extract.log"
    Start-Process msiexec.exe -Wait -ArgumentList "/a `"$msi`" /qn TARGETDIR=`"$extracted`" /L*v `"$log`""
}

$exe = Join-Path $extracted "PFiles64\Elgato\StreamDeck\StreamDeck.exe"
if (-not (Test-Path $exe)) {
    throw "StreamDeck.exe was not extracted; see $work\extract.log"
}

$file = Get-Item $exe
Write-Output "binary $($file.FullName) ($([math]::Round($file.Length/1MB,1)) MB)"
Write-Output "version $($file.VersionInfo.FileVersion)"
Write-Output "sha256 $((Get-FileHash $exe -Algorithm SHA256).Hash)"
Write-Output ""
Write-Output "next: target\debug\multideck-probe.exe strings `"$exe`" --grep vstreamdeck"
