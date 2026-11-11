;==============================================================================
;  Script d'installation Inno Setup pour ARXFUND
;  Application : ARXFUND (lecteur musical et studio de production natif)
;  Version     : 0.1.0
;
;  Pour compiler : iscc.exe installer\ARXFUND.iss
;  Résultat      : dist\ARXFUND-Setup-x64.exe
;==============================================================================

#define MyAppName "ARXFUND"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "ARXFUND"
#define MyAppURL "https://arxfund.example.com"
#define MyAppExeName "ARXFUND.exe"
; Identifiant d'application stable : NE JAMAIS modifier entre les versions,
; afin que les mises à jour remplacent l'installation existante et non
; n'en créent une en double.
#define MyAppId "{6F3A1C2E-8B4D-4F9A-9C2D-7E1B5A3D9F0C}"

[Setup]
; Nom de l'application affiché dans le Panneau de configuration.
AppId={{6F3A1C2E-8B4D-4F9A-9C2D-7E1B5A3D9F0C}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Dossier d'installation proposé : Program Files\ARXFUND (x64).
DefaultDirName={autopf64}\ARXFUND
DefaultGroupName=ARXFUND
AllowNoIcons=yes

; L'application elle-même s'exécute sans élévation (manifest asInvoker).
; L'installeur nécessite l'élévation uniquement pour écrire dans Program Files.
; L'utilisateur peut choisir un dossier utilisateur (ex. %LOCALAPPDATA%) pour
; éviter l'élévation durant l'installation.
PrivilegesRequired=admin

; Icône de l'installeur et de la désinstallation.
UninstallDisplayIcon={app}\{#MyAppExeName}

; Image de l'assistant d'installation (chemin relatif au dossier installer/).
WizardImageFile=wizard-large.bmp

; Architecture : Windows x64 uniquement.
ArchitecturesInstallIn64BitMode=x64
ArchitecturesAllowed=x64

; Compression optimisée.
Compression=lzma
SolidCompression=yes

; Fichier d'installation généré dans dist/ (relatif à installer/).
OutputDir=..\dist
OutputBaseFilename=ARXFUND-Setup-x64

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer un raccourci sur le Bureau"; GroupDescription: "Raccourcis"; Flags: unchecked
Name: "quickstarticon"; Description: "Lancer ARXFUND après l'installation"; GroupDescription: "Après l'installation"; Flags: unchecked

[Files]
; Exécutable principal et bibliothèques runtime Mingw (GNU).
Source: "..\target\release\ARXFUND.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "C:\msys64\mingw64\bin\libwinpthread-1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "C:\msys64\mingw64\bin\libgcc_s_seh-1.dll"; DestDir: "{app}"; Flags: ignoreversion

; Icône de l'application (également embarquée dans l'exécutable via build.rs).
Source: "..\assets\arxfund.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\assets\arxfund.png"; DestDir: "{app}\assets"; Flags: ignoreversion

; Ressources en lecture seule livrées avec l'application (presets, samples).
Source: "..\resources\presets\*"; DestDir: "{app}\resources\presets"; Flags: ignoreversion recursesubdirs
Source: "..\resources\samples\*"; DestDir: "{app}\resources\samples"; Flags: ignoreversion recursesubdirs

; Bitmaps de l'assistant d'installation (utilisées uniquement pendant l'installation).
Source: "wizard-large.bmp"; DestDir: "{app}"; Flags: ignoreversion
Source: "wizard-small.bmp"; DestDir: "{app}"; Flags: ignoreversion

; Manifeste et ressources Windows (référence, déjà embarquées dans l'exécutable).
Source: "..\resources\arxfund.manifest"; DestDir: "{app}\resources"; Flags: ignoreversion
Source: "..\resources\arxfund.rc"; DestDir: "{app}\resources"; Flags: ignoreversion

; Ne PAS inclure les données utilisateur :
;   %APPDATA%\ARXFUND\*        (base de données, paramètres, pochettes, sauvegardes)
;   %USERPROFILE%\Musique\ARXFUND\*  (projets, exports, enregistrements, samples, presets personnels)
; Ces dossiers sont créés à la première exécution par ARXFUND.

[Icons]
; Raccourci du menu Démarrer.
Name: "{group}\ARXFUND"; Filename: "{app}\{#MyAppExeName}"
; Raccourci Bureau (optionnel, coché par l'utilisateur).
Name: "{autodesktop}\ARXFUND"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
; Lancer ARXFUND après l'installation (optionnel).
Filename: "{app}\{#MyAppExeName}"; Description: "Lancer ARXFUND"; Flags: nowait postinstall skipifsilent; Tasks: quickstarticon

[UninstallDelete]
; Supprimer uniquement les ressources installées (les données utilisateur sont conservées).
Type: filesandordirs; Name: "{app}\resources\presets"
Type: filesandordirs; Name: "{app}\resources\samples"
Type: filesandordirs; Name: "{app}\assets"
