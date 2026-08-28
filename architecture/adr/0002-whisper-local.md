# ADR 0002 — Whisper local via whisper.cpp / whisper-rs

- **Statut** : Accepté
- **Date** : 2026-08-27
- **Décideur** : vitruve-agent (architecture)

## Contexte

La contrainte non-négociable « Offline total — aucune donnée ne quitte la machine » (app-spec.md) impose un moteur STT exécuté en local. La latence cible est < 2 s de la fin de parole à l'injection avec le modèle `base` (74 Mo) sur CPU 4 cœurs récent. Les langues cibles sont FR / EN / NL avec auto-détection.

## Options évaluées

### Option A — whisper.cpp via `whisper-rs`

- Portage C/C++ de Whisper OpenAI, optimisé CPU (SIMD, AVX2, AVX-512), Metal sur macOS, Vulkan/CUDA en option.
- Modèles GGML `tiny` (39 Mo), `base` (74 Mo), `small` (244 Mo), `medium` (769 Mo).
- Crate `whisper-rs` fournit un binding Rust stable et maintenu.
- 100 % offline après téléchargement du modèle.

### Option B — API cloud (OpenAI Whisper API, Azure Speech, Google STT)

- Meilleure qualité mais viole la contrainte offline non négociable.
- Coût récurrent, dépendance réseau, exfiltration audio.
- Éliminé d'office par contrainte spec.

### Option C — Vosk / Kaldi

- Offline, modèles plus petits, latence très basse.
- Qualité et ponctuation nettement inférieures à Whisper, surtout multilingue.
- Ponctuation à re-synthétiser côté app (non aligné sur la spec « Whisper gère la ponctuation »).

### Option D — Whisper natif Python via pyo3

- Ajoute une runtime Python de 40+ Mo à l'installer.
- Casse la NFR d'empreinte.

## Décision

Retenir **whisper.cpp via le crate `whisper-rs`** (option A).

Modèles proposés à l'utilisateur, par défaut `base` :

| Modèle | Taille | Latence 10 s audio (CPU 4c) | Qualité FR/EN/NL |
|---|---|---|---|
| tiny | 39 Mo | ~0.3 s | passable |
| base | 74 Mo | ~0.8 s | correcte (défaut) |
| small | 244 Mo | ~2.0 s | bonne (recommandé) |

## Conséquences

- **Positives** : offline strict, ponctuation gérée par le modèle, multilingue natif, latence maîtrisée.
- **Négatives** : premier lancement nécessite téléchargement du modèle (voir ADR 0005). Consommation RAM en dictée ~ 500 Mo avec `base`, ~ 1.2 Go avec `small`.
- **Risques** : qualité inférieure au cloud Whisper large-v3 — accepté car le compromis privacy/offline prime.

## Références

- ADR 0005 (téléchargement modèles HuggingFace)
- `architecture/vitruve-agent/2026-08-27_architecture-report.md` (section NFR)
