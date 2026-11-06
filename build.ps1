$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$env:RUSTUP_TOOLCHAIN = '1.99.1'
$rev = '62a01db8f006b422e6db24e34dcee05ad41760ba'
$artifact = 'target/horizon-mod/quick_place.hmod'
$toolDir = Join-Path ([IO.Path]::GetTempPath()) ('quick-place-tool-' + [guid]::NewGuid().ToString())
$oldPath = $env:PATH
$passed = $false
function Invoke-Checked([string]$program, [string[]]$cmdArgs) {
    & $program @cmdArgs
    if ($LASTEXITCODE -ne 0) { throw "$program failed with exit code $LASTEXITCODE" }
}
Remove-Item $artifact -Force -ErrorAction SilentlyContinue
try {
    Invoke-Checked 'rustup' @('toolchain','install',$env:RUSTUP_TOOLCHAIN,'--profile','minimal','--target','wasm32-unknown-unknown')
    Invoke-Checked 'cargo' @('install','--git','https://github.com/horizon-rust/horizon','--rev',$rev,'--locked','--root',$toolDir,'cargo-horizon-mod')
    $env:PATH = (Join-Path $toolDir 'bin') + [IO.Path]::PathSeparator + $oldPath
    Invoke-Checked 'cargo' @('test')
    Invoke-Checked 'cargo' @('horizon-mod','build')
    Invoke-Checked 'cargo' @('horizon-mod','test','--hmod',$artifact)
    Invoke-Checked 'cargo' @('horizon-mod','check',$artifact)
    if (-not (Test-Path $artifact) -or (Get-Item $artifact).Length -eq 0) { throw 'Missing or empty artifact' }
    $passed = $true
    Write-Host "Built and passed tests: $artifact"
} finally {
    $env:PATH = $oldPath
    if (-not $passed) { Remove-Item $artifact -Force -ErrorAction SilentlyContinue }
    Remove-Item $toolDir -Recurse -Force -ErrorAction SilentlyContinue
}
