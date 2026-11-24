# Captures the MultiDeck window to a PNG. Used to keep README and docs screenshots current.
#
#   powershell -ExecutionPolicy Bypass -File scripts/capture-window.ps1 -ProcessName multideck -Output assets/screenshots/editor.png
#
# The script waits for the window to appear, brings it to the foreground and captures
# the window rectangle with the desktop rasteriser. It never touches the content of
# the app itself, only the pixels the window manager draws.

param(
    [string]$ProcessName = "multideck",
    [Parameter(Mandatory = $true)][string]$Output,
    [int]$TimeoutSeconds = 30
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WindowTools {
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$process = $null
while ((Get-Date) -lt $deadline) {
    $process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } |
        Select-Object -First 1
    if ($process) { break }
    Start-Sleep -Milliseconds 250
}

if (-not $process) {
    Write-Error "window for process '$ProcessName' did not appear within $TimeoutSeconds seconds"
    exit 1
}

$handle = $process.MainWindowHandle
[void][WindowTools]::ShowWindow($handle, 9)
[void][WindowTools]::SetForegroundWindow($handle)
Start-Sleep -Milliseconds 700

$rect = New-Object WindowTools+RECT
if (-not [WindowTools]::GetWindowRect($handle, [ref]$rect)) {
    Write-Error "could not read the window rectangle"
    exit 1
}

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) {
    Write-Error "window has an empty rectangle"
    exit 1
}

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
$printed = [WindowTools]::PrintWindow($handle, $hdc, 2)
$graphics.ReleaseHdc($hdc)
if (-not $printed) {
    Write-Warning "PrintWindow failed, falling back to a screen capture"
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
}
$graphics.Dispose()

$directory = Split-Path -Parent $Output
if ($directory -and -not (Test-Path $directory)) {
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
}

$bitmap.Save((Resolve-Path -LiteralPath (Split-Path -Parent $Output)).Path + "\" + (Split-Path -Leaf $Output), [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()

Write-Output "captured ${width}x${height} to $Output"
