$ErrorActionPreference = 'Stop'

$grepRoot = Split-Path -Parent $PSScriptRoot
$grepDirectory = Join-Path $grepRoot 'target\profiling'
[void][System.IO.Directory]::CreateDirectory($grepDirectory)

$grepEncoding = [System.Text.UTF8Encoding]::new($false)
$grepBuilder = [System.Text.StringBuilder]::new()

for ($grepIndex = 0; $grepIndex -lt 8192; $grepIndex++) {
    if ($grepIndex % 64 -eq 0) {
        [void]$grepBuilder.Append("Rust makes ownership explicit.`n")
    } else {
        [void]$grepBuilder.Append("Small steps build understanding.`n")
    }
}

$grepChunk = $grepEncoding.GetBytes($grepBuilder.ToString())

if ($grepChunk.Length -ne 270080) {
    throw 'Generated chunk has an unexpected size.'
}

function Write-RepeatedChunk {
    param(
        [string]$Destination,
        [byte[]]$Chunk,
        [int]$Copies
    )

    $grepStream = [System.IO.File]::Create($Destination)
    try {
        for ($grepCopy = 0; $grepCopy -lt $Copies; $grepCopy++) {
            $grepStream.Write($Chunk, 0, $Chunk.Length)
        }
    } finally {
        $grepStream.Dispose()
    }
}

$grepSmallPath = Join-Path $grepDirectory 'profile-input.txt'
$grepLargePath = Join-Path $grepDirectory 'profile-input-large.txt'
$grepLongPath = Join-Path $grepDirectory 'profile-input-long-line.txt'

Write-RepeatedChunk -Destination $grepSmallPath `
    -Chunk $grepChunk -Copies 64

Write-RepeatedChunk -Destination $grepLargePath `
    -Chunk $grepChunk -Copies 512

$grepPrefix = [System.Text.Encoding]::ASCII.GetBytes('Rust')
$grepPadding = [System.Text.Encoding]::ASCII.GetBytes(('x' * 65536))
$grepRemaining = 17285120L - $grepPrefix.Length - 1

$grepStream = [System.IO.File]::Create($grepLongPath)
try {
    $grepStream.Write($grepPrefix, 0, $grepPrefix.Length)

    while ($grepRemaining -gt 0) {
        $grepAmount = [int][math]::Min(
            $grepRemaining, $grepPadding.Length
        )
        $grepStream.Write($grepPadding, 0, $grepAmount)
        $grepRemaining -= $grepAmount
    }

    $grepStream.WriteByte(10)
} finally {
    $grepStream.Dispose()
}

$grepSpecs = @(
    [pscustomobject]@{
        Path = $grepSmallPath
        Bytes = 17285120L
        ExpectedMatchingLines = 8192L
    }
    [pscustomobject]@{
        Path = $grepLargePath
        Bytes = 138280960L
        ExpectedMatchingLines = 65536L
    }
    [pscustomobject]@{
        Path = $grepLongPath
        Bytes = 17285120L
        ExpectedMatchingLines = 1L
    }
)

$grepResults = foreach ($grepSpec in $grepSpecs) {
    $grepFile = Get-Item -LiteralPath $grepSpec.Path

    if ($grepFile.Length -ne $grepSpec.Bytes) {
        throw "Unexpected file size: $($grepSpec.Path)"
    }

    [pscustomobject]@{
        File = $grepFile.Name
        Bytes = $grepFile.Length
        ExpectedMatchingLines = $grepSpec.ExpectedMatchingLines
    }
}

$grepResults | Format-Table -AutoSize
