# Installation et déploiement d'ARXFUND

Ce document explique comment compiler ARXFUND, générer l'installateur Windows,
l'installer sur un autre ordinateur, le désinstaller et le mettre à jour.

## 1. Prérequis de compilation (PC de développement)

Pour construire ARXFUND et son installateur, le PC de développement doit disposer de :

- **Rust** (toolchain stable) : https://rustup.dev/
- **Cible Windows x64 GNU** : `rustup target add x86_64-pc-windows-gnu`
- **MinGW-w64** : `C:\msys64\mingw64\bin` dans le PATH (pour `rusqlite` bundled et `windres`)
- **Inno Setup 7** : https://jrsoftware.org/isdl.php (compilateur `ISCC.exe`)
  - Alternative : `winget install --id JRSoftware.InnoSetup.7 -e`

```powershell
# Configurer le PATH pour la session
$env:Path = "C:\msys64\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:Path"

# Vérifier l'environnement
cargo --version
rustc --version
iscc.exe --version
```

## 2. Compiler ARXFUND (mode Release)

```powershell
cargo build --release
```

L'exécutable est produit ici :

```
target/release/ARXFUND.exe
```

Il est statiquement lié à la plupart de ses dépendances. Deux bibliothèques
runtime Mingw (`libwinpthread-1.dll`, `libgcc_s_seh-1.dll`) sont copiées à côté
par l'installateur.

## 3. Générer l'installateur Windows

Le script PowerShell automatise toute la chaîne de production :

```powershell
# Depuis la racine du dépôt
.\build-installer.ps1
```

Ce script :

1. Vérifie la présence de `cargo`, `rustc`, la cible `x86_64-pc-windows-gnu` et `ISCC.exe`.
2. Compile ARXFUND en mode Release (`cargo build --release`).
3. Vérifie l'existence de `target/release/ARXFUND.exe`.
4. Prépare les ressources (icône, presets, samples, bitmaps de l'assistant).
5. Compile le script Inno Setup (`installer/ARXFUND.iss`).
6. Génère le fichier d'installation final.
7. Vérifie son existence et sa cohérence.

**Fichier produit :**

```
dist/ARXFUND-Setup-x64.exe
```

Ce fichier est un véritable installateur Windows autonome (environ 8 Mo). Il peut
être copié sur une clé USB et transféré sur un autre PC.

## 4. Installer ARXFUND sur un autre ordinateur

Le PC de destination **n'a pas besoin** de :

- Rust, Cargo, OpenCode, DeepSeek
- Visual Studio
- Un environnement de développement
- Une connexion Internet (fonctions locales hors ligne)

### Procédure d'installation

1. Copiez `dist/ARXFUND-Setup-x64.exe` sur le PC de destination (clé USB, réseau, etc.).
2. Double-cliquez sur `ARXFUND-Setup-x64.exe`.
3. L'assistant d'installation s'ouvre (« Bienvenue dans l'assistant d'installation d'ARXFUND »).
4. Choisissez le dossier d'installation (par défaut : `Program Files\ARXFUND`).
5. Cochez éventuellement « Créer un raccourci sur le Bureau ».
6. Les raccourcis du menu Démarrer sont créés automatiquement.
7. Cochez éventuellement « Lancer ARXFUND après l'installation ».
8. Cliquez sur « Installer ».
9. À la fin, cliquez sur « Terminer ».

ARXFUND est alors disponible dans :

- Le menu Démarrer : `ARXFUND`
- Le Bureau (si l'option a été cochée)
- Paramètres Windows → Applications → Applications installées : `ARXFUND`

## 5. Désinstallation

ARXFUND apparaît dans :

```
Paramètres Windows → Applications → Applications installées → ARXFUND → Désinstaller
```

La désinstallation :

- Supprime l'exécutable et les fichiers installés dans le dossier d'installation.
- Supprime les raccourcis créés (menu Démarrer et Bureau).
- **Conserve toutes les données personnelles** :
  - Projets musicaux (`%USERPROFILE%\Musique\ARXFUND\Projects\`)
  - Exports (`%USERPROFILE%\Musique\ARXFUND\Exports\`)
  - Enregistrements (`%USERPROFILE%\Musique\ARXFUND\Recordings\`)
  - Samples importés (`%USERPROFILE%\Musique\ARXFUND\Samples\`)
  - Presets personnels (`%USERPROFILE%\Musique\ARXFUND\Presets\`)
  - Base de données, paramètres et pochettes (`%APPDATA%\ARXFUND\`)

## 6. Emplacement des données utilisateur

| Type de donnée | Emplacement |
|---|---|
| Base de données (bibliothèque, paramètres) | `%APPDATA%\ARXFUND\library.sqlite` |
| Pochettes d'albums | `%APPDATA%\ARXFUND\covers\` |
| Sauvegardes automatiques | `%APPDATA%\ARXFUND\backups\` |
| Projets musicaux | `%USERPROFILE%\Musique\ARXFUND\Projects\` |
| Exports WAV | `%USERPROFILE%\Musique\ARXFUND\Exports\` |
| Enregistrements | `%USERPROFILE%\Musique\ARXFUND\Recordings\` |
| Samples importés | `%USERPROFILE%\Musique\ARXFUND\Samples\` |
| Presets personnels | `%USERPROFILE%\Musique\ARXFUND\Presets\` |

Aucune donnée utilisateur modifiable n'est écrite dans `Program Files`.

## 7. Mise à jour d'ARXFUND

Pour installer une nouvelle version (ex. 0.2.0 par-dessus 0.1.0) :

1. Exécutez le nouvel installateur `ARXFUND-Setup-x64.exe`.
2. L'installeur reconnaît l'installation existante (même `AppId`) et remplace les fichiers du programme.
3. Vos paramètres, bibliothèque, projets et presets sont préservés.
4. Une migration de base de données est appliquée si nécessaire.

Ne désinstallez pas la version précédente avant d'installer la nouvelle : cela
n'est pas nécessaire et l'installeur gère la mise à jour directement.

## 8. Dépannage

### ARXFUND ne se lance pas

- Vérifiez qu'un périphérique audio est connecté. Si aucun n'est disponible,
  ARXFUND affiche un message clair mais reste utilisable (bibliothèque, export).
- Si l'icône de l'application ne s'affiche pas, c'est sans incidence sur le
  fonctionnement (l'icône est également embarquée dans l'exécutable).
- Consultez le journal d'erreurs Windows (Observateur d'événements) en cas de
  plantage au démarrage.

### Aucun périphérique audio

ARXFUND démarre en mode silencieux : la bibliothèque et l'export restent
fonctionnels. Branchez ou activez un périphérique audio, puis relancez ARXFUND.

### L'installateur demande les droits administrateur

L'élévation est nécessaire uniquement pour écrire dans `Program Files`. Vous
pouvez choisir un dossier utilisateur (par exemple `%LOCALAPPDATA%\Programs\ARXFUND`)
pendant l'installation pour éviter l'élévation.

### Fichier d'installation non signé

L'installateur n'est pas signé numériquement. Windows Defender ou SmartScreen
peut afficher un avertissement lors du premier lancement. Cliquez sur
« Informations complémentaires » → « Exécuter quand même ». Cela n'affecte en
rien le fonctionnement du logiciel.

## 9. Sécurité

- Aucun secret, token ou clé API n'est inclus dans l'installateur.
- Aucun téléchargement de composants depuis des sources non vérifiées.
- Windows Defender n'est pas désactivé.
- SmartScreen n'est pas contourné.
- Aucun fichier personnel du PC de développement n'est inclus.

## 10. Fichiers produits

| Fichier | Description |
|---|---|
| `target/release/ARXFUND.exe` | Application ARXFUND compilée (Release) |
| `dist/ARXFUND-Setup-x64.exe` | Installateur Windows autonome |
| `installer/ARXFUND.iss` | Script source Inno Setup |
| `build-installer.ps1` | Script PowerShell d'automatisation du build |
