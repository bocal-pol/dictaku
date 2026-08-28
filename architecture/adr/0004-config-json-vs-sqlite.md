# ADR 0004 — Configuration JSON en v0.1, SQLite prévu en v0.2

- **Statut** : Accepté
- **Date** : 2026-08-27
- **Décideur** : vitruve-agent (architecture)

## Contexte

Dictaku v0.1 doit persister :

- langue de transcription (auto / fr / en / nl),
- modèle Whisper actif (`tiny` / `base` / `small`),
- hotkey personnalisé (défaut `Ctrl+Alt+D`),
- préférences UI (thème, densité),
- chemin des modèles téléchargés + checksum.

v0.1 n'implémente **pas** l'historique persistant des dictées (hors périmètre spec). v0.2+ ajoutera :

- historique des dictées avec recherche plein texte,
- statistiques d'usage locales,
- dictionnaire personnel de corrections.

## Options évaluées

### Option A — JSON unique dans dossier config OS

- Chemin résolu via crate `directories` :
  - Windows : `%APPDATA%\Dictaku\config.json`
  - Linux : `~/.config/dictaku/config.json`
  - macOS : `~/Library/Application Support/Dictaku/config.json`
- Sérialisation via `serde_json` + typage strict via `serde::{Serialize, Deserialize}`.
- Écriture atomique (fichier temporaire + rename) pour éviter la corruption sur crash.
- Migration : champ `schema_version` dans le JSON, upgrade programmatique.

### Option B — SQLite dès v0.1

- Surdimensionné pour ~ 10 clés de configuration.
- Ajoute une dépendance (crate `rusqlite` + libsqlite embarqué) qui gonfle l'installer.
- Complexifie le debug utilisateur (le fichier n'est plus lisible avec un éditeur texte).

### Option C — TOML ou YAML

- TOML plus lisible que JSON mais serde-json est déjà présent (Tauri).
- YAML introduit une dépendance supplémentaire pour un gain marginal.

## Décision

**v0.1 : JSON via `serde_json`** (option A).

**v0.2 : ajout SQLite** dédié à l'historique et aux stats, la config reste JSON. Séparation stricte config / données.

## Justification

- La spec liste ~ 5 clés de config ; SQLite serait du gold-plating.
- JSON permet l'édition manuelle par l'utilisateur avancé (débogage sans lancer l'app).
- v0.2 réintroduit SQLite pour un besoin réel (recherche plein texte + agrégations) → décision différée cohérente.

## Conséquences

- **Positives** : v0.1 minimaliste, aucun install natif SQLite, config lisible.
- **Négatives** : migration à prévoir en v0.2 (importer les clés config existantes).
- **Risques** : concurrence d'écriture si l'utilisateur ouvre deux instances — mitigé par lock fichier + refus double-lancement (mutex nommé Windows, flock Linux).

## Références

- `architecture/vitruve-agent/2026-08-27_architecture-report.md` (module `config::settings`)
- Spec v0.1 (hors périmètre : historique persistant)
