<#
  Builds Kata Dev, Alex's daily copy, as an MSIX package, and installs it over the one in use.

    powershell -ExecutionPolicy Bypass -File packaging\build-msix.ps1

  Its own identity (AlexPalacios.Kata.Dev, shown as "Kata Dev") keeps it apart from the Store's
  Kata, which phase 6 adds as a second channel. Signed with the "Alex Palacios Dev" certificate,
  made on this PC and trusted only here; Developer Mode is never needed. Every build gets a fourth
  number higher than the installed one, so it installs over it. Settings stay: Windows keeps the
  package's registry apart, and an update never touches it.

  The package is written to target\msix\out\.
#>
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path $repo "target\msix"
$Name = "AlexPalacios.Kata.Dev"
$Publisher = "CN=Alex Palacios Dev"
$DisplayName = "Kata Dev"
# The Dev mark: the plate behind the glyph is orange.
$Plate = [System.Drawing.Color]::FromArgb(0xCA, 0x50, 0x10)

# --- Version: Cargo's 0.1.0 becomes the four numbers MSIX wants, 0.1.0.<n>, one past whatever is
# installed.
$numeric = (Select-String -Path (Join-Path $repo "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$revision = 1
$installed = Get-AppxPackage -Name $Name
if ($installed) {
    $v = [version]$installed.Version
    if ("$($v.Major).$($v.Minor).$($v.Build)" -eq $numeric) { $revision = $v.Revision + 1 }
}
$version = "$numeric.$revision"

# --- Build.
Push-Location $repo
try { cargo build --release -p kata; if ($LASTEXITCODE -ne 0) { throw "cargo build failed" } } finally { Pop-Location }

$stage = Join-Path $work "layout"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$assets = Join-Path $stage "Assets"
New-Item -ItemType Directory -Force $assets | Out-Null
Copy-Item (Join-Path $repo "target\release\Kata.exe") $stage

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
    $g.FillPath((New-Object System.Drawing.SolidBrush $Plate), $path)

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

    $bitmap.Save((Join-Path $assets $file), [System.Drawing.Imaging.ImageFormat]::Png)
    $pen.Dispose(); $glyph.Dispose(); $path.Dispose(); $g.Dispose(); $bitmap.Dispose()
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

# --- Manifest.
$manifest = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot "AppxManifest.xml")
$manifest = $manifest.Replace('$NAME$', $Name).Replace('$PUBLISHER$', $Publisher).
    Replace('$VERSION$', $version).Replace('$DISPLAYNAME$', $DisplayName)
[System.IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding($false)))

# --- resources.pri: the index that leads Windows from Assets\Square44x44Logo.png in the manifest to
# the right size among the logos above. Without it Windows finds none of them.
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1
if (-not $sdk) { throw "makeappx.exe not found: install the Windows SDK" }
$makepri = Join-Path $sdk.DirectoryName "makepri.exe"
$priconfig = Join-Path $work "priconfig.xml"
& $makepri createconfig /cf $priconfig /dq es-MX_en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed" }
# By default each scale goes to its own .pri, meant for a bundle. This is one package.
$config = [xml](Get-Content -Raw $priconfig)
$packaging = $config.SelectSingleNode("//packaging")
if ($packaging) { $packaging.ParentNode.RemoveChild($packaging) | Out-Null }
$config.Save($priconfig)
& $makepri new /pr $stage /cf $priconfig /mn (Join-Path $stage "AppxManifest.xml") /of (Join-Path $stage "resources.pri") /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri new failed" }

# --- Pack.
$outDir = Join-Path $work "out"
New-Item -ItemType Directory -Force $outDir | Out-Null
$out = Join-Path $outDir "$DisplayName $version.msix"
& $sdk.FullName pack /d $stage /p $out /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }

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
    if (-not (Trusted)) { throw "this PC does not trust the Alex Palacios Dev certificate, so Windows will not install Kata Dev" }
}

# --- Install over the previous Kata Dev. Windows closes it if it is open.
Add-AppxPackage -Path $out -ForceApplicationShutdown -ForceUpdateFromAnyVersion
Write-Host "installed: $DisplayName $version"
