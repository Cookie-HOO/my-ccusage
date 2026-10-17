$ErrorActionPreference = 'Stop'

$repo = 'Cookie-HOO/ccusage-dsh'
$installDir = if ($env:CCUSAGE_DSH_INSTALL_DIR) {
    $env:CCUSAGE_DSH_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\ccusage-dsh'
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'Only 64-bit Windows is supported.'
}
if ($env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'x86')) {
    throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE"
}

$asset = 'ccusage-dsh-windows-x64.zip'
$releaseUrl = "https://github.com/$repo/releases/latest/download"
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("ccusage-dsh-" + [guid]::NewGuid())
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
    $destination = Join-Path $installDir 'ccusage-dsh.exe'
    Copy-Item (Join-Path $tempDir 'ccusage-dsh-windows-x64\ccusage-dsh.exe') $destination -Force

    & $destination dsh --help | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Installed binary failed the DSH verification.' }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    if ($pathEntries -notcontains $installDir) {
        $newPath = (@($pathEntries) + $installDir) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    }
    if (($env:Path -split ';') -notcontains $installDir) {
        $env:Path = "$env:Path;$installDir"
    }

    Write-Host "Installed ccusage-dsh to $destination"
    Write-Host 'Open a new terminal, then run: ccusage-dsh dsh daily'
} finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
