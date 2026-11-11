# RUST MUSIC STUDIO — Journal de progression

Application musicale **native** 100 % Rust : bibliothèque locale, lecteur audio
et (à venir) mixage DJ et studio de remix.

## PHASE 1 — Application native ✅ TERMINÉE

### Ce qui fonctionne réellement

| Fonction | État |
|---|---|
| Application native Rust (egui/eframe), dark mode violet | ✅ |
| Moteur audio cpal (sortie f32, thread dédié) | ✅ |
| Décodage symphonia 0.6 (MP3, FLAC, WAV, OGG/Vorbis, AAC, M4A) | ✅ |
| Ring buffer SPSC lock-free (tests de concurrence) | ✅ |
| Rééchantillonnage linéaire si fréquence source ≠ périphérique | ✅ |
| Contrôles : lecture, pause, précédent, suivant, seek, volume | ✅ |
| File d'attente, aléatoire, répétition (off / file / piste) | ✅ |
| Reprise de lecture persistée (dernière piste + position) | ✅ |
| Volume persisté entre les sessions | ✅ |
| Base SQLite (rusqlite bundled) : songs, playlists, playlist_tracks, settings | ✅ |
| Import de fichiers : sélection, dossier récursif, glisser-déposer | ✅ |
| Métadonnées via lofty (titre, artiste, album, durée, pochette) | ✅ |
| Dédoublonnage par empreinte SHA-256 du contenu | ✅ |
| Barre de progression d'import + compteurs | ✅ |
| Bibliothèque : tri, filtre instantané, suppression, favoris | ✅ |
| Recherche SQLite (titre, artiste, album) | ✅ |
| Playlists : création, suppression, ajout/retrait, lecture | ✅ |
| Page Mixage DJ : état « non implémenté » affiché clairement (Phase 3) | ✅ |
| Page Studio de production (DAW Phase 1) : transport, Channel Rack, export | ✅ |
| Paramètres : version, chemins, confidentialité | ✅ |
| Tests : ring buffer, base de données, décodeur audio | ✅ |

### Limitations connues (Phase 1)

- **Resampling** : interpolation linéaire (pas d'anti-repliement). `rubato` est
  prévu pour la Phase 2 afin d'obtenir une qualité hi-fi.
- **Contrôles multimédias Windows (SMTC)** : non branchés (Phase 2).
- **Visualisations audio** : le graphe expose un `Analyser`-équivalent à venir ;
  aucune visualisation temps réel pour l'instant (Phase 4).
- **Paroles** : non implémentées (Phase 4).
- **wma / aiff** : non importés (non décodables par symphonia avec les features actuelles).
- **Mixage DJ** : page volontairement sans contrôles factices.

### DAW Phase 1 — Studio de production ✅ IMPLÉMENTÉE

| Fonction | État |
|---|---|
| Module `src/studio/` (projet, séquenceur, synthèse, échantillons, moteur, export, état, UI) | ✅ |
| Modèle de projet sérialisé JSON (`.rmsproj`) : canaux, patterns, pas, clips, invariants | ✅ |
| 10 instruments procéduraux : Kick, Snare, Clap, Charley fermé/ouvert, Perc, Rimshot, Cowbell, 808, Échantillon + métronome Click | ✅ |
| Channel Rack personnalisé : grille 16/32/64 pas, couleurs, mute/solo/volume/pan/pitch par canal | ✅ |
| Pas : activation, vélocité, probabilité, décalage, ratchet (1–4) | ✅ |
| Swing, signature rythmique, BPM 40–300, mode Pattern / Song (arrangement minimal) | ✅ |
| Moteur temps réel séparé du lecteur : rendu thread dédié + ring buffer, callback sans alloc | ✅ |
| Débutant : contrôles simplifiés, aide contextuelle, beat de démonstration interactif | ✅ |
| Sauvegarde/chargement projet, auto-sauvegarde 5 s, undo/redo (64 snapshots), scan de projets | ✅ |
| Export WAV offline (même séquenceur/synth que la lecture), 16/24/32 bits, normalisation, queue de fichier, import optionnel en bibliothèque | ✅ |
| Crosse-pause lecteur ↔ studio (pas de chevauchement audio) | ✅ |
| Tests : invariants projet, roundtrip serde, synthèse finie/non silencieuse, timing pas, export durée/non-silence/déterminisme | ✅ |

### Limitations connues (DAW Phase 1)

- **Enregistrement audio** : bouton présent mais désolé — Phase 7 (marqué « non implémenté »).
- **Éditeur d'arrangement complet** : mode Song utilise un seul clip du pattern actif ; édition multipiste différée à Phase 4.
- **Snap / grille magnétique** : différé à Phase 3.
- **Deux périphériques cpal simultanés** : dépendent du pilote ; en cas d'échec, une erreur s'affiche dans l'UI studio.
- **Échantillons Sample** : décodés synchrone à l'import (fichier court recommandé).

### Commandes

```powershell
cargo run          # lancer l'application
cargo check        # vérification
cargo test         # tests unitaires et d'intégration
cargo build --release
```

> Prérequis : toolchain Rust GNU (`rustup` + MinGW) dans le PATH.
> `C:\msys64\mingw64\bin` doit être dans le PATH pour la compilation de rusqlite.

## REFACTORING UI/UX ✅ TERMINÉ

Application d'une véritable identité visuelle "ARXFUND DARK" avec système de design centralisé :

| Fonction | État |
|---|---|
| Design system centralisé (`src/ui/`) : thème, palette, typographie, composants | ✅ |
| Composants réutilisables : Button, IconButton, Slider, Knob, Toggle, SearchField, Card, EmptyState, TrackHeader, AudioMeter, SectionHeader | ✅ |
| Music Player inspiré Liner/Spotify : sidebar, accueil, bibliothèque, pages album/playlist, barre de lecture permanente | ✅ |
| Palette ARXFUND DARK : fond #0D0F14, accent violet #8B5CF6, texte #F3F4F6 | ✅ |
| Typographie hiérarchisée : Display, H1, H2, Body, Label, Caption, Mono | ✅ |
| Studio de production : transport, Channel Rack, Piano Roll (placeholder), Playlist (placeholder), Mixer | ✅ |
| Mode débutant / Expert conservé | ✅ |

## PHASE 1.5 — Installateur Windows ✅ TERMINÉE

| Fonction | État |
|---|---|
| Installateur Windows Inno Setup (`dist/ARXFUND-Setup-x64.exe`) | ✅ |
| Script d'installation `installer/ARXFUND.iss` | ✅ |
| Script d'automatisation `build-installer.ps1` | ✅ |
| Documentation `INSTALLATION.md` | ✅ |
| Icône ARXFUND originale (`assets/arxfund.ico`, `.png`) | ✅ |
| Métadonnées Windows (VERSIONINFO, manifeste, `asInvoker`) | ✅ |
| Raccourcis menu Démarrer + Bureau | ✅ |
| Désinstallation préservant les données utilisateur | ✅ |
| Identifiant d'application stable pour les mises à jour | ✅ |
| Données utilisateur hors Program Files (%APPDATA%, Musique) | ✅ |
| Fonctionnement hors ligne après installation | ✅ |

## PHASE 2 — Bibliothèque et playlists ⏳ À FAIRE

- [ ] Réordonnancement des playlists (glisser-déposer)
- [ ] Fusion et export de playlists (M3U, JSON)
- [ ] Import M3U / CSV / JSON
- [ ] Intégration des métadonnées de plateformes (API officielles uniquement)
- [ ] Contrôles multimédias Windows (SMTC)
- [ ] Resampling haute qualité (rubato)
- [ ] Visualisations audio (spectrum FFT, VU-mètre)

## PHASE 3 — Mixage DJ ⏳ À FAIRE

- [ ] Deux platines (symphonia + cpal), crossfader, EQ, gain
- [ ] BPM, points CUE, boucles, formes d'onde
- [ ] Beatmatching manuel / automatique, beatgrid, hot cues
- [ ] Enregistrement du mix

## PHASE 4 — Studio de remix multipiste ⏳ À FAIRE

- [ ] Timeline multipiste, découpe, déplacement, superposition
- [ ] Effets modulaires (EQ, filtres, delay, reverb, distortion…)
- [ ] Automations, sauvegarde de projet, export WAV
- [ ] Visualisations avancées, paroles synchronisées

## PHASE 5 — Avancé ⏳ À FAIRE

- [ ] Time stretching, pitch shifting avancé
- [ ] Séparation voix/instruments (IA locale, structure prévue)
- [ ] MIDI, sampler, séquenceur

## PHASE 6 — Finalisation ⏳ À FAIRE

- [ ] Tests, optimisation, gestion des erreurs
- [ ] Installation Windows, documentation

---

# OPÉRATION RECONSTRUCTION — 80 ÉTAPES (ajout 11 nov. 2026)

## Résumé
Reconstruction méthodique de l'interface et des fonctionnalités audio suivantes l'audit initial. L'application démarre, compile et passe tous les tests. Le cœur (lecteur, studio, synthé, séquenceur, export WAV, sauvegarde) est réel et testé.

## Fichiers modifiés durant la reconstruction
| Fichier | Modification |
|---------|--------------|
| `src/studio/ui.rs` | Réécriture complète (transport, Channel Rack éditable, Piano Roll réel, Playlist, Mixer, export, aide, raccourcis, tutoriel) |
| `src/ui/audio_components.rs` | Réécriture propre (Knob, fader vertical, VU-mètre, BPM, mute/solo) |
| `src/studio/project.rs` | Ajout `ChannelKind::Synth`, struct `Note`, `SynthParams`, `Channel.tone`/`synth`, stockage des notes par pattern, serialisation |
| `src/studio/synth.rs` | Voix Synth ADSR, fréquence par note, glide, filtre tone, champs Trigger étendus (channel, frequency, hold_secs, synth, tone) |
| `src/studio/sequencer.rs` | Déclenchement des notes mélodiques, suivi des pics par canal (`channel_peaks`, `master_peak`) |
| `src/studio/engine.rs` | `channel_level(i)` et `master_level()` réels (atomiques partagés avec le thread de rendu) |
| `ARXFUND_80_STEPS_REPORT.md` | Rapport détaillé des 80 étapes (créé) |
| `ARXFUND_FINAL_STATUS.md` | État final et verdict (créé) |

## État de validation (étape 80)
- `cargo fmt --check` ✅
- `cargo check` ✅
- `cargo clippy` ✅ (warnings uniquement)
- `cargo test` ✅ (30 tests passés : synthèse, séquenceur, export WAV, projet, bibliothèque)
- `cargo build --release` ✅ (`target/release/ARXFUND.exe` 20,6 Mo)
- Lancement réel ✅ (fenêtre ARXFUND active)

## Verdict
**ALPHA fonctionnel** : le cœur est réel et testé, mais l'application n'est pas prête à la distribution (effets audio EQ/Reverb/Delay manquants, seek lecteur absent, finition graphique partielle, installateur non régénéré car Inno Setup indisponible).

