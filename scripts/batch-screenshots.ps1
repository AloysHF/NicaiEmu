<#
.SYNOPSIS
    Capture screenshots for CBE files.

.DESCRIPTION
    Runs the standalone emulator for every CBE file under the selected game
    directory. PNG screenshots are written to docs/images. When no binary is
    supplied, the latest release binary is built before capture.

.PARAMETER Frames
    Number of frames to run before capturing. Default: 120.

.PARAMETER InstructionLimit
    Maximum guest instructions allowed during boot and each frame.
    Default: 5000000.

.PARAMETER Binary
    Path to the nicaiemu executable. Default: the release build output.

.PARAMETER GameDirectory
    Directory searched recursively for CBE files.

.PARAMETER OutputDirectory
    Directory where PNG screenshots are written.
#>

param(
    [ValidateRange(0, [int]::MaxValue)]
    [int]$Frames = 120,
    [ValidateRange(1, [uint64]::MaxValue)]
    [uint64]$InstructionLimit = 5000000,
    [string]$Binary = "",
    [string]$GameDirectory = "",
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $GameDirectory) {
    $GameDirectory = Join-Path $repoRoot "tmp\nicai_game"
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repoRoot "docs\images"
}
if (-not (Test-Path -LiteralPath $GameDirectory -PathType Container)) {
    Write-Error "Game directory not found: $GameDirectory"
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

if (-not $Binary) {
    $Binary = Join-Path $repoRoot "target\release\nicaiemu.exe"
    if ($env:OS -ne "Windows_NT") {
        $Binary = Join-Path $repoRoot "target/release/nicaiemu"
    }

    Write-Host "Building the latest release binary..." -ForegroundColor Yellow
    try {
        Push-Location $repoRoot
        cargo build --release -p nicaiemu --bin nicaiemu
        if ($LASTEXITCODE -ne 0) {
            throw "Release build failed with exit code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    Write-Error "nicaiemu binary not found: $Binary"
    exit 1
}

$games = @(Get-ChildItem -LiteralPath $GameDirectory -Recurse -File |
    Where-Object { $_.Extension -ieq ".cbe" } |
    Sort-Object FullName)

if ($games.Count -eq 0) {
    Write-Warning "No CBE files found under $GameDirectory"
    exit 0
}

function Get-SafeBaseName {
    param(
        [Parameter(Mandatory)]
        [string]$BaseName
    )

    $safeName = $BaseName -replace '[<>:"/\\|?*\x00-\x1F]', '_'
    $safeName = $safeName.Trim().TrimEnd('.')
    if (-not $safeName) {
        return "unnamed"
    }
    return $safeName
}

Write-Host "Using binary: $Binary"
Write-Host "Game dir:     $GameDirectory"
Write-Host "Output dir:   $OutputDirectory"
Write-Host "Frames:       $Frames"
Write-Host "Games:        $($games.Count)"
Write-Host ""

$success = 0
$warned = 0
$failed = 0
$usedNames = [System.Collections.Generic.Dictionary[string, int]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)

foreach ($game in $games) {
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($game.Name)
    $safeName = Get-SafeBaseName $baseName
    if ($usedNames.ContainsKey($safeName)) {
        $usedNames[$safeName]++
        $safeName = "$safeName-$($usedNames[$safeName])"
    } else {
        $usedNames[$safeName] = 1
    }

    $imageName = "$safeName.png"
    $outPath = Join-Path $OutputDirectory $imageName
    Write-Host -NoNewline "  $baseName ... "

    # Prevent a previous capture from being reported as the current result.
    if (Test-Path -LiteralPath $outPath -PathType Leaf) {
        Remove-Item -LiteralPath $outPath
    }

    $status = "Fail"
    $detail = "Execution failed"
    $nonzero = $null
    $colors = $null
    $exitCode = -1
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Binary --file $game.FullName --screenshot $outPath `
            --screenshot-frames $Frames --instruction-limit $InstructionLimit 2>&1
        $exitCode = $LASTEXITCODE
        $diagnostics = $output | Out-String
        $diagnostics = [regex]::Replace(
            $diagnostics,
            '\x1B\[[0-?]*[ -/]*[@-~]',
            ''
        )
        if ($diagnostics -match 'frame nonzero=(\d+) colors=(\d+)') {
            $nonzero = [int]$Matches[1]
            $colors = [int]$Matches[2]
        }

        if ($exitCode -ne 0) {
            $diagnosticLines = @($diagnostics -split '\r?\n' |
                ForEach-Object { $_.Trim() } |
                Where-Object { $_ })
            if ($diagnosticLines.Count -gt 0) {
                $detail = $diagnosticLines[$diagnosticLines.Count - 1]
                $detail = $detail -replace '^\d+:\s*', ''
                $detail = $detail -replace '^Error:\s*', ''
            } else {
                $detail = "Exited with code $exitCode"
            }
            Write-Host $detail -ForegroundColor Red
        } elseif (-not (Test-Path -LiteralPath $outPath -PathType Leaf)) {
            $detail = "No screenshot produced"
            Write-Host $detail -ForegroundColor Red
        } elseif ($null -eq $nonzero -or $null -eq $colors) {
            $status = "Warn"
            $detail = "Frame statistics unavailable"
            Write-Host $detail -ForegroundColor Yellow
        } elseif ($nonzero -eq 0 -or $colors -le 1) {
            $status = "Warn"
            $detail = "Blank or single-color frame"
            Write-Host "$detail ($colors colors)" -ForegroundColor Yellow
        } else {
            $status = "Pass"
            $detail = "Rendered $colors colors"
            $size = (Get-Item -LiteralPath $outPath).Length
            Write-Host "OK ($colors colors, $([math]::Round($size / 1KB)) KB)" -ForegroundColor Green
        }
    } catch {
        $detail = $_.Exception.Message
        Write-Host "FAILED ($detail)" -ForegroundColor Red
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    switch ($status) {
        "Pass" { $success++ }
        "Warn" { $warned++ }
        default { $failed++ }
    }
}

Write-Host ""
Write-Host "Done: $success succeeded, $warned warned, $failed failed out of $($games.Count) total."
