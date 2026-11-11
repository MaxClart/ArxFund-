# RUST MUSIC STUDIO

Application musicale **native** écrite en Rust : bibliothèque locale unifiée,
lecteur audio haute fidélité et (à venir) mixage DJ + studio de remix.

> Projet indépendant. Aucun code, ressource ou élément distinctif de service
> tiers n'est repris.

## Technologies

- **Rust** (toolchain GNU sur Windows) — langage principal
- **egui / eframe 0.37** — interface graphique native
- **cpal 0.18** — sortie audio (WASAPI sur Windows)
- **symphonia 0.6** — décodage (MP3, FLAC, WAV, OGG/Vorbis, AAC, M4A)
- **rusqlite 0.41 (bundled)** — base de données locale
- **lofty 0.25** — métadonnées et pochettes
- **sha2** — empreintes de fichiers (dédoublonnage)
- **rfd** — boîtes de dialogue natives
- **image** — décodage des pochettes
- **hound 3.5** — écriture WAV (export du studio)

## Compilation

Prérequis : toolchain Rust + MinGW dans le PATH.

```powershell
# Ajouter MinGW au PATH pour la session
$env:Path = "C:\msys64\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:Path"

cargo check
cargo test
cargo run
cargo build --release
```

## Structure

```
rust-music-studio/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── app.rs          # état applicatif + UI (eframe::App)
│   ├── audio/
│   │   ├── mod.rs
│   │   ├── engine.rs   # moteur cpal + thread de décodage
│   │   └── ring.rs     # ring buffer SPSC lock-free
│   └── library/
│       ├── mod.rs
│       ├── db.rs       # rusqlite (schéma + requêtes)
│       ├── models.rs   # Song, Playlist, Provider
│       └── import.rs   # import de fichiers + métadonnées
│   └── studio/
│       ├── mod.rs
│       ├── engine.rs   # stream cpal + thread de rendu du séquenceur
│       ├── export.rs   # rendu hors ligne WAV (16/24/32 bits)
│       ├── project.rs  # modèle de projet (.rmsproj) + scan
│       ├── samples.rs  # décodage échantillons vers mono
│       ├── sequencer.rs# séquenceur pas à pas + synthèse des voix
│       ├── state.rs    # état projet, undo/redo, auto-sauvegarde, export
│       ├── synth.rs    # instruments de batterie procéduraux
│       └── ui.rs       # transport, Channel Rack, export, aide débutant
├── tests/
│   ├── library_test.rs
│   └── decoder_test.rs
├── PROGRESS.md
└── README.md
```

## Données utilisateur

- Base : `%APPDATA%\rust-music-studio\library.sqlite`
- Pochettes : `%APPDATA%\rust-music-studio\covers\`
- Projets studio : `%USERPROFILE%\Musique\rust-music-studio\projects\` (fichiers `.rmsproj`)
- Exports WAV : `%USERPROFILE%\Musique\rust-music-studio\exports\`

Aucun compte requis, aucune télémétrie, aucun secret dans le code.

## Droits et plateformes musicales

Utilisation exclusive d'API officielles, d'exports utilisateurs et de fichiers
légalement accessibles. Aucun contournement de DRM, aucun téléchargement de flux
protégé. Les intégrations tierces ne seront affichées comme fonctionnelles que
lorsqu'elles le seront réellement.

## État du projet

Voir `PROGRESS.md`.
