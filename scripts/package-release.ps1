#Requires -Version 7.0

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet(
        'x86_64-pc-windows-msvc',
        'x86_64-unknown-linux-gnu',
        'x86_64-apple-darwin',
        'aarch64-apple-darwin'
    )]
    [string]$Target,

    [ValidatePattern('^(local|[0-9a-fA-F]{40}|[0-9a-fA-F]{64})$')]
    [string]$SourceRevision = 'local'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Invoke-CheckedNative {
    param(
        [scriptblock]$Action,
        [string]$FailureMessage
    )

    $grepCaptured = @(& $Action)

    if ($LASTEXITCODE -ne 0) {
        throw $FailureMessage
    }

    $grepCaptured
}

$grepRoot = Split-Path -Parent $PSScriptRoot
$grepManifest = Join-Path $grepRoot 'Cargo.toml'

$grepMetadataText = Invoke-CheckedNative {
    cargo +stable metadata --locked --no-deps --format-version 1 --manifest-path $grepManifest
} 'Cannot read Cargo metadata.'

$grepMetadata = ($grepMetadataText -join "`n") | ConvertFrom-Json
$grepPackages = @(
    $grepMetadata.packages |
        Where-Object { $_.name -eq 'mini-grep' }
)

if ($grepPackages.Count -ne 1) {
    throw 'Expected exactly one mini-grep package.'
}

$grepVersion = [string]($grepPackages[0].version)

$grepRustInfo = @(Invoke-CheckedNative {
    rustc +stable --version --verbose
} 'Cannot read Rust compiler information.')

$grepHostLines = @(
    $grepRustInfo | Where-Object { $_ -like 'host: *' }
)

if (
    $grepHostLines.Count -ne 1 -or
    $grepHostLines[0] -cne "host: $Target"
) {
    throw 'Packaging checks require a native build for the requested target.'
}

$grepCargoVersion = Invoke-CheckedNative {
    cargo +stable --version
} 'Cannot read Cargo version.'

$grepAboutVersion = Invoke-CheckedNative {
    cargo +stable about --version
} 'Cannot read cargo-about version.'

$grepSuffix = if ($IsWindows) { '.exe' } else { '' }
$grepBinaryName = "mini-grep$grepSuffix"
$grepBundleName = "mini-grep-$grepVersion-$Target"
$grepExtension = if ($IsWindows) { '.zip' } else { '.tar.gz' }

$grepOutputRoot = Join-Path $grepRoot 'target/release-artifacts'
$grepArchive = Join-Path $grepOutputRoot "$grepBundleName$grepExtension"
$grepChecksum = "$grepArchive.sha256"

if (
    (Test-Path -LiteralPath $grepArchive) -or
    (Test-Path -LiteralPath $grepChecksum)
) {
    throw "Release output already exists: $grepArchive"
}

$grepBinary = Join-Path $grepRoot "target/$Target/release/$grepBinaryName"

$grepFiles = [ordered]@{
    $grepBinaryName = $grepBinary
    'README.md' = (Join-Path $grepRoot 'README.md')
    'CHANGELOG.md' = (Join-Path $grepRoot 'CHANGELOG.md')
    'LICENSE-MIT' = (Join-Path $grepRoot 'LICENSE-MIT')
    'LICENSE-APACHE' = (Join-Path $grepRoot 'LICENSE-APACHE')
    'THIRD-PARTY-NOTICES.txt' = (
        Join-Path $grepRoot 'target/THIRD-PARTY-NOTICES.txt'
    )
    'docs/cli-contract.md' = (
        Join-Path $grepRoot 'docs/cli-contract.md'
    )
}

foreach ($grepSource in $grepFiles.Values) {
    $grepItem = Get-Item -LiteralPath $grepSource

    if ($grepItem.PSIsContainer -or $grepItem.Length -eq 0) {
        throw "Required release file is empty or is a directory: $grepSource"
    }
}

$grepWorkRoot = Join-Path $grepRoot (
    'target/release-staging/' + [guid]::NewGuid().ToString('N')
)
$grepStageParent = Join-Path $grepWorkRoot 'stage'
$grepStage = Join-Path $grepStageParent $grepBundleName
$grepExtractRoot = Join-Path $grepWorkRoot 'extracted'

[System.IO.Directory]::CreateDirectory($grepStage) | Out-Null
[System.IO.Directory]::CreateDirectory($grepExtractRoot) | Out-Null
[System.IO.Directory]::CreateDirectory($grepOutputRoot) | Out-Null

foreach ($grepEntry in $grepFiles.GetEnumerator()) {
    $grepDestination = Join-Path $grepStage $grepEntry.Key
    $grepParent = Split-Path -Parent $grepDestination

    [System.IO.Directory]::CreateDirectory($grepParent) | Out-Null
    Copy-Item -LiteralPath $grepEntry.Value -Destination $grepDestination
}

$grepEncoding = [System.Text.UTF8Encoding]::new($false)
$grepLockHash = (
    Get-FileHash -LiteralPath (Join-Path $grepRoot 'Cargo.lock') -Algorithm SHA256
).Hash.ToLowerInvariant()

$grepBuildInfo = @(
    "package: mini-grep $grepVersion"
    "target: $Target"
    "source_revision: $SourceRevision"
    "cargo_lock_sha256: $grepLockHash"
    "cargo: $grepCargoVersion"
    "cargo_about: $grepAboutVersion"
    ''
    'rustc:'
) + $grepRustInfo

[System.IO.File]::WriteAllText(
    (Join-Path $grepStage 'BUILD-INFO.txt'),
    (($grepBuildInfo -join "`n") + "`n"),
    $grepEncoding
)

$grepStageBinary = Join-Path $grepStage $grepBinaryName

if ($IsWindows) {
    Compress-Archive -LiteralPath $grepStage -DestinationPath $grepArchive
    Expand-Archive -LiteralPath $grepArchive -DestinationPath $grepExtractRoot
} else {
    Invoke-CheckedNative {
        chmod +x $grepStageBinary
    } 'Cannot set executable permission.' | Out-Null

    Invoke-CheckedNative {
        tar -czf $grepArchive -C $grepStageParent $grepBundleName
    } 'Cannot create release archive.' | Out-Null

    Invoke-CheckedNative {
        tar -xzf $grepArchive -C $grepExtractRoot
    } 'Cannot extract release archive.' | Out-Null
}

$grepExtractedBundle = Join-Path $grepExtractRoot $grepBundleName
$grepRelativeFiles = @($grepFiles.Keys) + @('BUILD-INFO.txt')

foreach ($grepRelativeFile in $grepRelativeFiles) {
    $grepOriginalHash = Get-FileHash -Algorithm SHA256 -LiteralPath (
        Join-Path $grepStage $grepRelativeFile
    )
    $grepExtractedHash = Get-FileHash -Algorithm SHA256 -LiteralPath (
        Join-Path $grepExtractedBundle $grepRelativeFile
    )

    if ($grepOriginalHash.Hash -cne $grepExtractedHash.Hash) {
        throw "Extracted file differs from staged file: $grepRelativeFile"
    }
}

$grepExtractedBinary = Join-Path $grepExtractedBundle $grepBinaryName
$grepProbe = Join-Path $grepExtractRoot 'probe.txt'

[System.IO.File]::WriteAllText(
    $grepProbe,
    "before`nRust Rust`nafter`n",
    $grepEncoding
)

$grepNoMatchOutput = @(
    & $grepExtractedBinary --no-config --color=never -F -h missing $grepProbe 2>&1
)

if ($LASTEXITCODE -ne 1 -or $grepNoMatchOutput.Count -ne 0) {
    throw 'Extracted executable failed the no-match check.'
}

$grepActualVersion = @(Invoke-CheckedNative {
    & $grepExtractedBinary --version 2>&1
} 'Extracted executable version check failed.')

if (($grepActualVersion -join "`n") -cne "mini-grep $grepVersion") {
    throw 'Extracted executable returned an unexpected version.'
}

$grepSearchOutput = @(Invoke-CheckedNative {
    & $grepExtractedBinary --no-config --color=never -F -n -h Rust $grepProbe 2>&1
} 'Extracted executable search failed.')

if (($grepSearchOutput -join "`n") -cne '2:Rust Rust') {
    throw 'Extracted executable returned unexpected search output.'
}

$grepArchiveHash = (
    Get-FileHash -LiteralPath $grepArchive -Algorithm SHA256
).Hash.ToLowerInvariant()

$grepArchiveFileName = Split-Path -Leaf $grepArchive

[System.IO.File]::WriteAllText(
    $grepChecksum,
    "$grepArchiveHash  $grepArchiveFileName`n",
    $grepEncoding
)

[pscustomobject]@{
    Archive = $grepArchive
    Checksum = $grepChecksum
    SHA256 = $grepArchiveHash
}