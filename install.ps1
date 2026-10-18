$ErrorActionPreference = 'Stop'

$repo = 'Cookie-HOO/my-ccusage'
$installDir = if ($env:MY_CCUSAGE_INSTALL_DIR) {
    $env:MY_CCUSAGE_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\my-ccusage'
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'Only 64-bit Windows is supported.'
}
if ($env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'x86')) {
    throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE"
}

$asset = 'my-ccusage-windows-x64.zip'
$releaseUrl = "https://github.com/$repo/releases/latest/download"
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("my-ccusage-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

try {
    $archive = Join-Path $tempDir $asset
    $checksums = Join-Path $tempDir 'SHA256SUMS'
    Invoke-WebRequest "$releaseUrl/$asset" -OutFile $archive
    Invoke-WebRequest "$releaseUrl/SHA256SUMS" -OutFile $checksums

    $line = Get-Content $checksums | Where-Object { $_ -match "\s$([regex]::Escape($asset))$" } | Select-Object -First 1
    if (-not $line) { throw "Checksum for $asset was not found." }
    $expected = ($line -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "SHA-256 mismatch for $asset." }

    Expand-Archive $archive -DestinationPath $tempDir -Force
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    $destination = Join-Path $installDir 'my-ccusage.exe'
    Copy-Item (Join-Path $tempDir 'my-ccusage-windows-x64\my-ccusage.exe') $destination -Force

    & $destination dsh --help | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Installed binary failed the DSH verification.' }
    & $destination workbuddy --help | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Installed binary failed the WorkBuddy verification.' }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    if ($pathEntries -notcontains $installDir) {
        $newPath = (@($pathEntries) + $installDir) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    }
    if (($env:Path -split ';') -notcontains $installDir) {
        $env:Path = "$env:Path;$installDir"
    }

    Write-Host "Installed my-ccusage to $destination"
    Write-Host 'Open a new terminal, then run: my-ccusage dsh daily'
} finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
