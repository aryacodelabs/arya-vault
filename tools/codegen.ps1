# Windows counterpart of tools/codegen.sh.
#   tools/codegen.ps1           regenerate
#   tools/codegen.ps1 -Check    regenerate and fail if anything differs from what is committed
param([switch]$Check)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Get-Content tools/codegen/FRB_VERSION -Raw).Trim()

# The pin check is a bash script; Git for Windows ships bash.
bash tools/codegen/check-pins.sh
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

foreach ($tool in 'cargo', 'dart', 'flutter') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Error "'$tool' is not on PATH"
        exit 2
    }
}

$have = $null
if (Get-Command flutter_rust_bridge_codegen -ErrorAction SilentlyContinue) {
    $have = ((flutter_rust_bridge_codegen --version) -split '\s+')[-1]
}
if ($have -ne $version) {
    cargo install flutter_rust_bridge_codegen --version "=$version" --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Push-Location app
flutter_rust_bridge_codegen generate
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { exit $code }

if ($Check) {
    git diff --exit-code -- app/lib/src/rust core/crates/ffi/src/frb_generated.rs
    if ($LASTEXITCODE -ne 0) {
        Write-Error 'the generated bindings are out of date; run tools/codegen.ps1 and commit'
        exit 1
    }
    $untracked = git ls-files --others --exclude-standard -- app/lib/src/rust core/crates/ffi/src/frb_generated.rs
    if ($untracked) {
        Write-Error "generated files are not committed: $untracked"
        exit 1
    }
    Write-Host 'generated bindings are up to date'
}
