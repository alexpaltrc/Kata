<#
  Builds Kata Dev as an MSIX package and installs it over the copy in use.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1 [-Store]

  Kata Dev (AlexPalacios.Kata.Dev, its icon labeled "Dev") is where every change is tried, apart in
  every way from Kata, the copy Alex uses every day. It is signed with the "Alex Palacios Dev"
  certificate, made on this PC and trusted only here; Developer Mode is never needed.

  With -Store, it builds Kata for the Microsoft Store instead: the identity Partner Center reserved,
  x64 and ARM64 in one bundle, unsigned (the Store signs it) and not installed. Upload it as is.
  Kata comes only from the Store, and Alex updates it himself.

  Packages are written to target\msix\out\.
#>
param([switch]$Store)
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
} else {
    $Name = "AlexPalacios.Kata.Dev"
    $DisplayName = "Kata Dev"
}
$Targets = @{ x64 = "x86_64-pc-windows-msvc"; arm64 = "aarch64-pc-windows-msvc" }

# --- Version: MSIX wants four numbers. A Store release is Cargo's 0.1.0 as 0.1.0.0, so releasing
# means raising it in Cargo.toml; a Dev build is 0.1.0.<n>, one past the Dev installed.
$numeric = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if ($Store) {
    $version = "$numeric.0"
} else {
    $revision = 1
    $installed = Get-AppxPackage -Name $Name
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

# --- Logos: Kata's glyph, the four corners of the tray icon (crates/ui/src/icon.rs), around a lit
# area: the Focus. No plate, like Windows 11's own icons, in Fluent blues lit from the top left.
# `share` is how much of the image the glyph's 16-unit grid takes, which tiles keep small.
$Bright = [System.Drawing.Color]::FromArgb(0x3A, 0xB0, 0xFF)
$Deep = [System.Drawing.Color]::FromArgb(0x00, 0x5A, 0xC8)
$Glow = [System.Drawing.Color]::FromArgb(0xD2, 0xF0, 0xFF)
$Lit = [System.Drawing.Color]::FromArgb(0x7F, 0xCF, 0xFF)

function Rounded([single]$left, [single]$top, [single]$width, [single]$height, [single]$radius) {
    $d = 2 * $radius
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($left, $top, $d, $d, 180, 90)
    $path.AddArc($left + $width - $d, $top, $d, $d, 270, 90)
    $path.AddArc($left + $width - $d, $top + $height - $d, $d, $d, 0, 90)
    $path.AddArc($left, $top + $height - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $path
}

function Logo([string]$file, [int]$size, [double]$share) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = "AntiAlias"
    $g.PixelOffsetMode = "HighQuality"

    # Edges fall on whole pixels, so the smallest sizes stay crisp.
    $side = $size * $share
    $origin = ($size - $side) / 2
    $unit = $side / 16
    $snap = { param([double]$units) [single][Math]::Round($origin + $units * $unit) }
    [single]$stroke = [Math]::Max(1, [Math]::Round(2 * $unit))
    [single]$near = (& $snap 1) + $stroke / 2
    [single]$far = $size - $near
    [single]$arm = (& $snap 6.5) - $stroke / 2 - $near
    [single]$r = 2 * $unit
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
    # A gradient a pixel past the grid, so its ends never wrap around.
    $from = New-Object System.Drawing.PointF ($origin - 1), ($origin - 1)
    $to = New-Object System.Drawing.PointF ($origin + $side + 1), ($origin + $side + 1)
    $corners = New-Object System.Drawing.Drawing2D.LinearGradientBrush $from, $to, $Bright, $Deep
    $pen = New-Object System.Drawing.Pen $corners, $stroke
    $pen.StartCap = "Round"
    $pen.EndCap = "Round"
    $g.DrawPath($pen, $glyph)

    [single]$inset = & $snap 5
    $area = Rounded $inset $inset ($size - 2 * $inset) ($size - 2 * $inset) (1.5 * $unit)
    $focus = New-Object System.Drawing.Drawing2D.LinearGradientBrush $from, $to, $Glow, $Lit
    $g.FillPath($focus, $area)

    if (-not $Store) {
        # The Dev mark, as on the tray icon: a label across the foot that says "Dev", kept apart
        # from the corners it covers by a clear gap.
        [single]$left = & $snap 0
        [single]$edge = $size - $left
        [single]$height = [Math]::Round(7 * $unit)
        [single]$label = $edge - $height
        [single]$gap = [Math]::Max(1, [Math]::Round($unit))
        $g.SetClip((New-Object System.Drawing.RectangleF $left, ($label - $gap), ($edge - $left), ($height + $gap)))
        $g.Clear([System.Drawing.Color]::Transparent)
        $g.ResetClip()
        $tag = Rounded $left $label ($edge - $left) $height (2 * $unit)
        $fill = New-Object System.Drawing.SolidBrush $Deep
        $g.FillPath($fill, $tag)
        # Centered by its ink, which a word without descenders needs.
        $word = New-Object System.Drawing.Drawing2D.GraphicsPath
        $word.AddString("Dev", (New-Object System.Drawing.FontFamily "Segoe UI Semibold"), 0, 0.9 * $height,
            (New-Object System.Drawing.PointF 0, 0), [System.Drawing.StringFormat]::GenericTypographic)
        $ink = $word.GetBounds()
        $move = New-Object System.Drawing.Drawing2D.Matrix
        $move.Translate($left + ($edge - $left - $ink.Width) / 2 - $ink.X, $label + ($height - $ink.Height) / 2 - $ink.Y)
        $word.Transform($move)
        $g.FillPath([System.Drawing.Brushes]::White, $word)
        $move.Dispose(); $word.Dispose(); $fill.Dispose(); $tag.Dispose()
    }

    $bitmap.Save((Join-Path $assets $file), [System.Drawing.Imaging.ImageFormat]::Png)
    $focus.Dispose(); $area.Dispose(); $pen.Dispose(); $corners.Dispose(); $glyph.Dispose(); $g.Dispose(); $bitmap.Dispose()
}
foreach ($size in 16, 24, 32, 48, 256) {
    Logo "Square44x44Logo.targetsize-$size.png" $size 1
    Logo "Square44x44Logo.targetsize-${size}_altform-unplated.png" $size 1
}
foreach ($scale in 100, 200) {
    Logo "Square44x44Logo.scale-$scale.png" (44 * $scale / 100) 1
    Logo "Square150x150Logo.scale-$scale.png" (150 * $scale / 100) 0.5
    Logo "StoreLogo.scale-$scale.png" (50 * $scale / 100) 1
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
