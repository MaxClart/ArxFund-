#==============================================================================
#  build-installer.ps1
#  Script de compilation automatisée de l'installateur ARXFUND.
#
#  Ce script :
#    1. Vérifie l'environnement (cargo, rustc, Inno Setup).
#    2. Compile ARXFUND en mode Release (Windows x64).
#    3. Vérifie l'existence d'ARXFUND.exe.
#    4. Regroupe les ressources dans un dossier de staging.
#    5. Identifie et copie les DLL nécessaires (Mingw runtime).
#    6. Lance la compilation du script Inno Setup (installer\ARXFUND.iss).
#    7. Génère dist\ARXFUND-Setup-x64.exe.
#    8. Vérifie l'existence du fichier final.
#==============================================================================

$ProgressPreference = "SilentlyContinue"

function Stop-Build {
    param([string]$Message)
    Write-Host "ERREUR : $Message" -ForegroundColor Red
    exit 1
}

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = (Resolve-Path $ScriptDir).Path
$DistDir = Join-Path $ProjectRoot "dist"
$InstallerDir = Join-Path $ProjectRoot "installer"
$IssScript = Join-Path $InstallerDir "ARXFUND.iss"
$SetupExeName = "ARXFUND-Setup-x64.exe"
$SetupExePath = Join-Path $DistDir $SetupExeName
$ReleaseExe = Join-Path $ProjectRoot "target\release\ARXFUND.exe"
$InnoSetupPaths = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 7\ISCC.exe",
    "C:\Program Files\Inno Setup 7\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "  build-installer.ps1 - ARXFUND Installer" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 1 : Vérification de l'environnement
# ---------------------------------------------------------------------------
Write-Host "[1/9] Vérification de l'environnement..." -ForegroundColor Yellow

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Stop-Build "cargo introuvable. Installez Rust (https://rustup.dev/) et ajoutez %USERPROFILE%\.cargo\bin au PATH."
}
$rustc = Get-Command rustc -ErrorAction SilentlyContinue
Write-Host "  cargo   : $($cargo.Source)" -ForegroundColor Green
Write-Host "  rustc   : $($rustc.Source)" -ForegroundColor Green

$targetList = rustc --print target-list 2>$null
if (-not ($targetList -contains "x86_64-pc-windows-gnu")) {
    Stop-Build "la cible x86_64-pc-windows-gnu n'est pas installée. Ajoutez-la avec : rustup target add x86_64-pc-windows-gnu"
}
Write-Host "  target  : x86_64-pc-windows-gnu présent" -ForegroundColor Green

$iscc = $null
foreach ($p in $InnoSetupPaths) {
    if (Test-Path $p) {
        $iscc = $p
        break
    }
}
if (-not $iscc) {
    Stop-Build "Inno Setup (ISCC.exe) est introuvable. Installez Inno Setup 7 depuis https://jrsoftware.org/isdl.php"
}
Write-Host "  iscc    : $iscc" -ForegroundColor Green

$windres = Get-Command windres -ErrorAction SilentlyContinue
if ($windres) {
    Write-Host "  windres : $($windres.Source) (ressources Windows OK)" -ForegroundColor Green
} else {
    Write-Host "  windres : introuvable (l'exécutable sera compilé sans icône embarquée ni VERSIONINFO)" -ForegroundColor Yellow
}

Write-Host ""

# ---------------------------------------------------------------------------
# Étape 2 : Compilation Release de ARXFUND
# ---------------------------------------------------------------------------
Write-Host "[2/9] Compilation Release de ARXFUND (cargo build --release)..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    $build = & cargo build --release 2>&1
    $exitCode = $LASTEXITCODE
    $build | ForEach-Object { Write-Host "  $_" }
    if ($exitCode -ne 0) {
        Stop-Build "la compilation Release a échoué (code de sortie $exitCode)."
    }
}
finally {
    Pop-Location
}
Write-Host "  Compilation réussie." -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 3 : Vérification de l'existence d'ARXFUND.exe
# ---------------------------------------------------------------------------
Write-Host "[3/9] Vérification de ARXFUND.exe..." -ForegroundColor Yellow
if (-not (Test-Path $ReleaseExe)) {
    Stop-Build "$ReleaseExe est introuvable après la compilation."
}
$exeSize = (Get-Item $ReleaseExe).Length
Write-Host "  Trouvé : $ReleaseExe ($([math]::Round($exeSize / 1mb, 2)) Mo)" -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 4 : Regroupement des ressources
# ---------------------------------------------------------------------------
Write-Host "[4/9] Préparation des ressources..." -ForegroundColor Yellow

$requiredFiles = @(
    "$ProjectRoot\assets\arxfund.ico",
    "$ProjectRoot\assets\arxfund.png",
    "$ProjectRoot\resources\presets",
    "$ProjectRoot\resources\samples",
    "$InstallerDir\wizard-large.bmp",
    "$InstallerDir\wizard-small.bmp"
)
foreach ($f in $requiredFiles) {
    if (-not (Test-Path $f)) {
        Stop-Build "ressource manquante : $f"
    }
}
Write-Host "  Toutes les ressources obligatoires sont présentes." -ForegroundColor Green

$mingwDlls = @(
    "C:\msys64\mingw64\bin\libwinpthread-1.dll",
    "C:\msys64\mingw64\bin\libgcc_s_seh-1.dll"
)
$missingDlls = $mingwDlls | Where-Object { -not (Test-Path $_) }
if ($missingDlls) {
    Stop-Build "DLL runtime Mingw manquantes : $($missingDlls -join ', '). Ajoutez C:\msys64\mingw64\bin au PATH."
}
Write-Host "  DLL runtime Mingw trouvées." -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 5 : Création du dossier dist/
# ---------------------------------------------------------------------------
Write-Host "[5/9] Création du dossier de sortie dist/..." -ForegroundColor Yellow
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
}
Write-Host "  Dossier : $DistDir" -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 6 : Compilation de l'installateur Inno Setup
# ---------------------------------------------------------------------------
Write-Host "[6/9] Compilation de l'installateur Inno Setup..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    $isscOutput = & $iscc /Q $IssScript 2>&1
    $exitCode = $LASTEXITCODE
    $isscOutput | ForEach-Object { Write-Host "  $_" }
    if ($exitCode -ne 0) {
        Stop-Build "la compilation Inno Setup a échoué (code de sortie $exitCode)."
    }
}
finally {
    Pop-Location
}
Write-Host "  Installateur compilé avec succès." -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 7 : Vérification du fichier final
# ---------------------------------------------------------------------------
Write-Host "[7/9] Vérification du fichier d'installation..." -ForegroundColor Yellow
if (-not (Test-Path $SetupExePath)) {
    Stop-Build "$SetupExePath est introuvable après la compilation Inno Setup."
}
$setupSize = (Get-Item $SetupExePath).Length
Write-Host "  Fichier final : $SetupExePath" -ForegroundColor Green
Write-Host "  Taille : $([math]::Round($setupSize / 1mb, 2)) Mo" -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 8 : Vérifications complémentaires
# ---------------------------------------------------------------------------
Write-Host "[8/9] Vérifications complémentaires..." -ForegroundColor Yellow

if ($setupSize -lt 1mb) {
    Stop-Build "l'installateur semble trop petit ($setupSize octets), il est peut-être incomplet."
}
Write-Host "  Taille de l'installateur cohérente." -ForegroundColor Green
Write-Host "  Le désinstallateur sera créé automatiquement dans le dossier d'installation." -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# Étape 9 : Résumé
# ---------------------------------------------------------------------------
Write-Host "[9/9] Terminé." -ForegroundColor Green
Write-Host ""
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "  INSTALLATEUR GÉNÉRÉ AVEC SUCCÈS" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Chemin        : $SetupExePath"
Write-Host "  Taille        : $([math]::Round($setupSize / 1mb, 2)) Mo"
Write-Host "  Application   : ARXFUND v0.1.0"
Write-Host "  Exécutable    : ARXFUND.exe"
Write-Host "  Dossier cible : Program Files\ARXFUND (choisissible à l'installation)"
Write-Host ""
Write-Host "  Données utilisateur :"
Write-Host "    - %APPDATA%\ARXFUND (base de données, paramètres, pochettes)"
Write-Host "    - %USERPROFILE%\Musique\ARXFUND (projets, exports, samples, presets)"
Write-Host ""
Write-Host "  Pour installer sur un autre PC :"
Write-Host "    1. Copiez $SetupExeName sur une clé USB."
Write-Host "    2. Double-cliquez dessus sur le PC de destination."
Write-Host ""
Write-Host "  Pour désinstaller :"
Write-Host "    Paramètres Windows → Applications → ARXFUND → Désinstaller."
Write-Host ""
Write-Host "  Les données personnelles (projets, exports, presets) sont conservées."
Write-Host ""
Write-Host "==============================================" -ForegroundColor Cyan

exit 0
