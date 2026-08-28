# ADR 0005 — Téléchargement des modèles Whisper depuis HuggingFace

- **Statut** : Accepté
- **Date** : 2026-08-27
- **Décideur** : vitruve-agent (architecture)

## Contexte

La contrainte offline est stricte, **sauf** pour le téléchargement initial des modèles Whisper GGML. Les modèles pèsent 39 à 244 Mo (tiny à small) — les embarquer dans l'installer casserait la NFR « installer < 10 Mo ». Ils doivent donc être téléchargés au premier lancement (ou lors du changement de modèle depuis l'UI).

## Source retenue

**HuggingFace** — dépôt officiel `ggerganov/whisper.cpp` :

- URL type : `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin`
- Modèles vérifiés et maintenus par l'auteur de whisper.cpp.
- CDN performant et stable.

## Flux de téléchargement

```text
1. Utilisateur choisit un modèle (défaut = base)   →   UI Settings
2. Core Rust vérifie si le fichier existe localement
   ├─ si oui  → vérification SHA256 → OK → charger
   └─ si non  → phase download :
        a. GET streaming avec progress bar tray + WebView
        b. écriture dans dossier modèles OS-specific
        c. vérification SHA256 (checksum embarqué dans l'app)
        d. si checksum KO → suppression + erreur → retry manuel
        e. si checksum OK → renommage atomique (.part → .bin)
3. Chargement whisper-rs → prêt pour dictée
```

## Emplacement des modèles

- Windows : `%APPDATA%\Dictaku\models\ggml-<name>.bin`
- Linux : `~/.local/share/dictaku/models/ggml-<name>.bin`
- macOS : `~/Library/Application Support/Dictaku/models/ggml-<name>.bin`

Résolus via crate `directories` (`ProjectDirs::data_dir`).

## Sécurité et intégrité

- **Checksums SHA256** des versions supportées **embarqués dans le binaire** (constants Rust générées au build depuis un fichier `models_manifest.json` versionné).
- **HTTPS obligatoire** — refus explicite de `http://` (défense en profondeur, même si HuggingFace redirige).
- **Aucun call réseau après téléchargement** — flag applicatif `has_required_models` bloque tout appel HTTP hors du flux download explicitement déclenché par l'utilisateur.
- **Mode air-gapped** : possibilité de placer manuellement les fichiers `.bin` dans le dossier modèles avant premier lancement — Dictaku détecte, valide checksum, saute le download.

## Options écartées

### Option — Embarquer le modèle `tiny` dans l'installer

- Casse la NFR installer < 10 Mo (tiny = 39 Mo).
- Rejeté.

### Option — CDN privé maintenu par le projet

- Coût et responsabilité de maintenance.
- HuggingFace fournit déjà la source de vérité amont.
- Rejeté.

### Option — Torrent / IPFS

- Complexité disproportionnée pour v0.1, expérience utilisateur incertaine.
- Rejeté (reconsidérable en v0.3+ si volumétrie modèles explose).

## Conséquences

- **Positives** : installer léger, choix du modèle offert à l'utilisateur, checksums vérifiés.
- **Négatives** : premier lancement nécessite connexion internet (documenté clairement dans l'onboarding).
- **Risques** : rupture HuggingFace → prévoir un mirror configurable dans un futur ADR si le besoin apparaît.

## Références

- ADR 0002 (choix Whisper local)
- `architecture/vitruve-agent/2026-08-27_architecture-report.md` (module `stt::model_manager`)
