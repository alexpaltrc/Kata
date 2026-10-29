<#
  Builds Kata as an MSIX package and installs it over the copy in use.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1 [-Stable | -Store]

  Two copies, apart in every way: Kata Dev (AlexPalacios.Kata.Dev, its icon labeled "Dev"), where
  every change is tried, and, with -Stable, Kata (AlexPalacios.Kata), the one Alex uses every day,
  which changes only when he releases a version. Each identity installs only over itself, and
  Windows keeps each package's settings apart and untouched by updates.

  Both are signed with the "Alex Palacios Dev" certificate, made on this PC and trusted only here;
  Developer Mode is never needed.

  With -Store, it builds Kata for the Microsoft Store instead: the identity Partner Center reserved,
  x64 and ARM64 in one bundle, unsigned (the Store signs it) and not installed. Upload it as is.

  Packages are written to target\msix\out\.
#>
param([switch]$Stable, [switch]$Store)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path $repo "target\msix"
$Publisher = "CN=Alex Palacios Dev"
$Architectures = @("x64")
if ($Store) {
    # From Partner Center, Product identity.
    $Name = "AlexPalacios.Kata"
    $Publisher = "CN=A3B4D4CD-AECB-4B6D-9268-AEAB19F8F17B"
    $DisplayName = "Kata"
    $Architectures = @("x64", "arm64")
    $Stable = $true
} elseif ($Stable) {
    $Name = "AlexPalacios.Kata"
    $DisplayName = "Kata"
} else {
    $Name = "AlexPalacios.Kata.Dev"
    $DisplayName = "Kata Dev"
}
$Targets = @{ x64 = "x86_64-pc-windows-msvc"; arm64 = "aarch64-pc-windows-msvc" }
# Windows' own blue, until phase 6 gives Kata its identity.
$Plate = [System.Drawing.Color]::FromArgb(0x00, 0x5F, 0xB8)

# --- Version: MSIX wants four numbers. A stable release is Cargo's 0.1.0 as 0.1.0.0, so releasing
# means raising it in Cargo.toml; a Dev build is 0.1.0.<n>, one past the Dev installed.
$numeric = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$installed = if (-not $Store) { Get-AppxPackage -Name $Name }
if ($Stable) {
    $version = "$numeric.0"
    if ($installed -and [version]$installed.Version -ge [version]$version) {
        throw "Kata $($installed.Version) is already installed: raise the version in Cargo.toml"
    }
} else {
    $revision = 1
    if ($installed) {
        $v = [version]$installed.Version
        if ("$($v.Major).$($v.Minor).$($v.Build)" -eq $numeric) { $revision = $v.Revision + 1 }
    }
    $version = "$numeric.$revision"
}

# --- Build.
Push-Location $repo
try {
    foreach ($arch in $Architectures) {
        cargo build --release -p kata --target $Targets[$arch]
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    }
} finally { Pop-Location }

$stage = Join-Path $work "layout"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$assets = Join-Path $stage "Assets"
New-Item -ItemType Directory -Force $assets | Out-Null

# --- Logos: Kata's glyph, the four corners of crates/ui/src/icon.rs, in white on a rounded plate.
# `share` is how much of the image the plate takes, which tiles keep small.
function Logo([string]$file, [int]$size, [double]$share) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = "AntiAlias"
    $g.PixelOffsetMode = "HighQuality"

    $side = $size * $share
    [single]$top = ($size - $side) / 2
    [single]$corner = $side * 0.4
    [single]$bottom = $top + $side - $corner
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($top, $top, $corner, $corner, 180, 90)
    $path.AddArc($bottom, $top, $corner, $corner, 270, 90)
    $path.AddArc($bottom, $bottom, $corner, $corner, 0, 90)
    $path.AddArc($top, $bottom, $corner, $corner, 90, 90)
    $path.CloseFigure()
    $brush = New-Object System.Drawing.SolidBrush $Plate
    $g.FillPath($brush, $path)

    # The glyph on a 16-unit grid, as icon.rs draws it.
    $grid = $side * 0.62
    $origin = ($size - $grid) / 2
    $unit = $grid / 16
    $near = $origin + 2 * $unit
    $far = $origin + $grid - 2 * $unit
    $arm = 4.5 * $unit
    $r = 2 * $unit
    $glyph = New-Object System.Drawing.Drawing2D.GraphicsPath
    foreach ($c in @(@($near, $near, 1, 1), @($far, $near, -1, 1), @($far, $far, -1, -1), @($near, $far, 1, -1))) {
        $x, $y, $dx, $dy = $c
        $glyph.StartFigure()
        $glyph.AddLine([single]$x, [single]($y + $dy * $arm), [single]$x, [single]($y + $dy * $r))
        $start = if ($dx -gt 0) { 180 } else { 0 }
        $sweep = if ($dx * $dy -gt 0) { 90 } else { -90 }
        $glyph.AddArc([single]($x + $dx * $r - $r), [single]($y + $dy * $r - $r), [single](2 * $r), [single](2 * $r), $start, $sweep)
        $glyph.AddLine([single]($x + $dx * $r), [single]$y, [single]($x + $dx * $arm), [single]$y)
    }
    $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([single][Math]::Max(1, 1.25 * $unit))
    $g.DrawPath($pen, $glyph)

    if (-not $Stable) {
        # The Dev mark, as on the tray icon: the plate's lower part is a white label that says
        # "Dev", kept apart from the corners it covers by a strip of plate.
        [single]$height = [Math]::Round($side * 7 / 16)
        [single]$label = [Math]::Round($top + $side - $height)
        $lower = {
            param([single]$y)
            $part = New-Object System.Drawing.Drawing2D.GraphicsPath
            $part.AddLine($top, $y, $top + $side, $y)
            $part.AddArc($bottom, $bottom, $corner, $corner, 0, 90)
            $part.AddArc($top, $bottom, $corner, $corner, 90, 90)
            $part.CloseFigure()
            $part
        }
        $strip = & $lower ($label - [Math]::Max(1, [Math]::Round($side / 16)))
        $tag = & $lower $label
        $g.FillPath($brush, $strip)
        $g.FillPath([System.Drawing.Brushes]::White, $tag)
        # Centered by its ink, which a word without descenders needs.
        $word = New-Object System.Drawing.Drawing2D.GraphicsPath
        $word.AddString("Dev", (New-Object System.Drawing.FontFamily "Segoe UI Semibold"), 0, 0.9 * $height,
            (New-Object System.Drawing.PointF 0, 0), [System.Drawing.StringFormat]::GenericTypographic)
        $ink = $word.GetBounds()
        $move = New-Object System.Drawing.Drawing2D.Matrix
        $move.Translate($top + ($side - $ink.Width) / 2 - $ink.X, $label + ($height - $ink.Height) / 2 - $ink.Y)
        $word.Transform($move)
        $g.FillPath($brush, $word)
        $move.Dispose(); $word.Dispose(); $tag.Dispose(); $strip.Dispose()
    }

    $bitmap.Save((Join-Path $assets $file), [System.Drawing.Imaging.ImageFormat]::Png)
    $pen.Dispose(); $glyph.Dispose(); $brush.Dispose(); $path.Dispose(); $g.Dispose(); $bitmap.Dispose()
}
foreach ($size in 16, 24, 32, 48, 256) {
    Logo "Square44x44Logo.targetsize-$size.png" $size 0.875
    Logo "Square44x44Logo.targetsize-${size}_altform-unplated.png" $size 0.875
}
foreach ($scale in 100, 200) {
    Logo "Square44x44Logo.scale-$scale.png" (44 * $scale / 100) 0.875
    Logo "Square150x150Logo.scale-$scale.png" (150 * $scale / 100) 0.5
    Logo "StoreLogo.scale-$scale.png" (50 * $scale / 100) 0.875
}

$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $sdk) { throw "makeappx.exe not found: install the Windows SDK" }
$makepri = Join-Path $sdk.DirectoryName "makepri.exe"
$priconfig = Join-Path $work "priconfig.xml"
& $makepri createconfig /cf $priconfig /dq es-MX_en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed" }
# By default each scale goes to its own .pri, meant for resource packages. Each package keeps all.
$config = [xml](Get-Content -Raw $priconfig)
$packaging = $config.SelectSingleNode("//packaging")
if ($packaging) { $packaging.ParentNode.RemoveChild($packaging) | Out-Null }
$config.Save($priconfig)

$outDir = Join-Path $work "out"
New-Item -ItemType Directory -Force $outDir | Out-Null
$bundle = Join-Path $work "bundle"
if (Test-Path $bundle) { Remove-Item -Recurse -Force $bundle }
New-Item -ItemType Directory -Force $bundle | Out-Null

# --- One package per architecture: its Kata.exe, manifest and resources.pri around the same logos.
foreach ($arch in $Architectures) {
    Copy-Item (Join-Path $repo "target\$($Targets[$arch])\release\Kata.exe") $stage -Force

    $manifest = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot "AppxManifest.xml")
    $manifest = $manifest.Replace('$NAME$', $Name).Replace('$PUBLISHER$', $Publisher).
        Replace('$VERSION$', $version).Replace('$DISPLAYNAME$', $DisplayName).Replace('$ARCH$', $arch)
    [System.IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding($false)))

    # resources.pri: the index that leads Windows from Assets\Square44x44Logo.png in the manifest to
    # the right size among the logos above. Without it Windows finds none of them.
    Remove-Item (Join-Path $stage "resources.pri") -ErrorAction SilentlyContinue
    & $makepri new /pr $stage /cf $priconfig /mn (Join-Path $stage "AppxManifest.xml") /of (Join-Path $stage "resources.pri") /o | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "makepri new failed" }

    $out = if ($Store) { Join-Path $bundle "Kata_${version}_$arch.msix" } else { Join-Path $outDir "$DisplayName $version.msix" }
    & $sdk.FullName pack /d $stage /p $out /o | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
}

if ($Store) {
    $upload = Join-Path $outDir "Kata $version.msixbundle"
    & $sdk.FullName bundle /d $bundle /p $upload /bv $version /o | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "makeappx bundle failed" }
    Write-Host "for the Store: $upload"
    return
}

# --- Sign with the development certificate, made here the first time. Its private key never
# leaves this user's certificate store.
$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $Publisher } | Select-Object -First 1
if (-not $cert) {
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $Publisher `
        -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature `
        -FriendlyName "Alex Palacios Dev" -NotAfter (Get-Date).AddYears(10) `
        -TextExtension @("2.5.29.19={text}")
}
& (Join-Path $sdk.DirectoryName "signtool.exe") sign /fd SHA256 /sha1 $cert.Thumbprint /s My $out | Out-Null
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }

function Trusted { Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object { $_.Thumbprint -eq $cert.Thumbprint } }
if (-not (Trusted)) {
    # Once per PC: Windows installs a package only from a certificate in the machine's Trusted
    # People, and writing there takes one UAC prompt, answered by whoever is at the keyboard.
    $cer = Join-Path $work "dev.cer"
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-NoProfile -Command Import-Certificate -FilePath '$cer' -CertStoreLocation Cert:\LocalMachine\TrustedPeople"
    if (-not (Trusted)) { throw "this PC does not trust the Alex Palacios Dev certificate, so Windows will not install $DisplayName" }
}

# --- Install over the previous copy. Windows closes it if it is open.
Add-AppxPackage -Path $out -ForceApplicationShutdown -ForceUpdateFromAnyVersion
Write-Host "installed: $DisplayName $version"
