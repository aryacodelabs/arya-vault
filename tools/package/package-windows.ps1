# Builds the Windows release and packages it (L01 goal 9).
#
#   tools/package/package-windows.ps1                 build everything, write dist/
#   tools/package/package-windows.ps1 -SkipBuild      package the existing build (app/build/...)
#   tools/package/package-windows.ps1 -OutDir <path>  write somewhere else (default: <repo>/dist)
#
# Outputs, in the output directory:
#   AryaVault-<version>-windows-x64-portable.zip   the portable build (recommended for the preview)
#   AryaVault-<version>-windows-x64.msix           UNSIGNED MSIX (see docs/windows-install.md)
#   SHA256SUMS.txt                                 checksums of both
#
# Needs: cargo, flutter (both on PATH) and the Windows SDK's makeappx.exe for the MSIX.
# Nothing is signed (code signing is M9) and nothing is uploaded.
param(
    [switch]$SkipBuild,
    [string]$OutDir
)
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $OutDir) { $OutDir = Join-Path $root 'dist' }

# --- version: app/pubspec.yaml is the single source (kAppVersion is tested against it) -------
$pubspec = Get-Content (Join-Path $root 'app/pubspec.yaml') -Raw
if ($pubspec -notmatch '(?m)^version:\s*(\d+)\.(\d+)\.(\d+)\+(\d+)\s*$') {
    throw 'cannot read version: from app/pubspec.yaml'
}
$semver = "$($Matches[1]).$($Matches[2]).$($Matches[3])"
$msixVersion = "$semver.$($Matches[4])"

$bundle = Join-Path $root 'app/build/windows/x64/runner/Release'

if (-not $SkipBuild) {
    foreach ($tool in 'cargo', 'flutter') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "'$tool' is not on PATH" }
    }
    Push-Location (Join-Path $root 'core')
    try {
        cargo build -p arya-vault-ffi --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    } finally { Pop-Location }
    Push-Location (Join-Path $root 'app')
    try {
        flutter build windows --release
        if ($LASTEXITCODE -ne 0) { throw 'flutter build windows failed' }
    } finally { Pop-Location }
}

# --- the bundle must be complete: an app without its core must never be packaged ---------------
foreach ($required in 'arya_vault.exe', 'flutter_windows.dll', 'arya_vault_ffi.dll', 'data') {
    if (-not (Test-Path (Join-Path $bundle $required))) {
        throw "the build is incomplete: $required is missing from $bundle"
    }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$zipName = "AryaVault-$semver-windows-x64-portable.zip"
$msixName = "AryaVault-$semver-windows-x64.msix"
$zipPath = Join-Path $OutDir $zipName
$msixPath = Join-Path $OutDir $msixName
foreach ($old in $zipPath, $msixPath) { if (Test-Path -LiteralPath $old) { Remove-Item -LiteralPath $old -Force } }

# --- portable zip: one top-level folder, plus the notes ------------------------------------------
$stageRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("aryavault-pkg-" + [guid]::NewGuid().ToString('N'))
try {
    $zipStage = Join-Path $stageRoot 'AryaVault'
    New-Item -ItemType Directory -Force -Path $zipStage | Out-Null
    Copy-Item -Path (Join-Path $bundle '*') -Destination $zipStage -Recurse
    Copy-Item -LiteralPath (Join-Path $root 'docs/windows-install.md') -Destination (Join-Path $zipStage 'INSTALL.md')
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $zipStage 'LICENSE') -ErrorAction SilentlyContinue
    Compress-Archive -Path $zipStage -DestinationPath $zipPath -CompressionLevel Optimal

    # --- MSIX (unsigned) ---------------------------------------------------------------------------
    $makeappx = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
    if ($null -eq $makeappx) {
        Write-Warning 'makeappx.exe (Windows SDK) not found: the MSIX was not built'
    } else {
        $msixStage = Join-Path $stageRoot 'msix'
        New-Item -ItemType Directory -Force -Path $msixStage | Out-Null
        Copy-Item -Path (Join-Path $bundle '*') -Destination $msixStage -Recurse
        $assets = Join-Path $msixStage 'Assets'
        New-Item -ItemType Directory -Force -Path $assets | Out-Null

        # Placeholder tiles (a plain square with an "A"); real artwork comes with the brand work.
        Add-Type -AssemblyName System.Drawing
        foreach ($size in @(@('Square44x44Logo.png', 44), @('Square150x150Logo.png', 150), @('StoreLogo.png', 50))) {
            $bmp = New-Object System.Drawing.Bitmap $size[1], $size[1]
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            $g.Clear([System.Drawing.Color]::FromArgb(255, 36, 52, 99))
            $font = New-Object System.Drawing.Font 'Segoe UI', ([single]($size[1] * 0.55)), ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
            $fmt = New-Object System.Drawing.StringFormat
            $fmt.Alignment = [System.Drawing.StringAlignment]::Center
            $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
            $g.DrawString('A', $font, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 0, 0, $size[1], $size[1]), $fmt)
            $g.Dispose()
            $bmp.Save((Join-Path $assets $size[0]), [System.Drawing.Imaging.ImageFormat]::Png)
            $bmp.Dispose()
        }

        # Identity is a placeholder. Capabilities are the minimum for a full-trust desktop app.
        $manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="AryaCodeLabs.AryaVault" Publisher="CN=AryaCode Labs (unsigned preview)" Version="$msixVersion" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>AryaVault</DisplayName>
    <PublisherDisplayName>AryaCode Labs</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="AryaVault" Executable="arya_vault.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="AryaVault" Description="Local-first, zero-knowledge password and notes manager"
                          BackgroundColor="#243463" Square150x150Logo="Assets\Square150x150Logo.png"
                          Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
        Set-Content -LiteralPath (Join-Path $msixStage 'AppxManifest.xml') -Value $manifest -Encoding UTF8
        & $makeappx.FullName pack /d $msixStage /p $msixPath /o
        if ($LASTEXITCODE -ne 0) { throw 'makeappx pack failed' }
    }
} finally {
    if (Test-Path -LiteralPath $stageRoot) { Remove-Item -LiteralPath $stageRoot -Recurse -Force }
}

# --- checksums ----------------------------------------------------------------------------------
$lines = foreach ($f in $zipPath, $msixPath) {
    if (Test-Path -LiteralPath $f) {
        $h = (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash.ToLower()
        "$h  $(Split-Path -Leaf $f)"
    }
}
Set-Content -LiteralPath (Join-Path $OutDir 'SHA256SUMS.txt') -Value $lines -Encoding ASCII

Write-Host ''
Write-Host "version $semver (msix $msixVersion)"
$lines | ForEach-Object { Write-Host $_ }
Get-ChildItem $OutDir | Select-Object Name, @{n='MB'; e={[math]::Round($_.Length / 1MB, 1)}} | Format-Table -AutoSize
