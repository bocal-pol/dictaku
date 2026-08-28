# Data Dictionary — dictaku v0.1

## Périmètre v0.1

Une seule source de persistance : **fichier de configuration JSON local**.  
Aucune base de données. Aucun réseau. Aucun compte utilisateur.  
Toutes les données restent sur la machine de l'utilisateur.

---

## Fichier : `~/.dictaku/config.json`

Chemin effectif Windows : `C:\Users\<username>\.dictaku\config.json`

Créé automatiquement au premier lancement avec les valeurs par défaut.

### Schéma JSON

```json
{
  "hotkey": "ctrl+alt+d",
  "language": "auto",
  "whisper_model": "small",
  "models_path": "~/.dictaku/models",
  "audio": {
    "device": "default",
    "silence_timeout_ms": 3000,
    "sample_rate": 16000
  },
  "ui": {
    "tray_animation": true,
    "startup_with_windows": true
  }
}
```

### Dictionnaire des champs

| Champ | Type | Défaut | Obligatoire | Description | PII |
|---|---|---|---|---|---|
| `hotkey` | `string` | `"ctrl+alt+d"` | oui | Raccourci global d'activation. Format : touches séparées par `+`, minuscules. Valeurs acceptées : modificateurs (`ctrl`, `alt`, `shift`, `win`) + touche alphanumérique. | Non |
| `language` | `string` (enum) | `"auto"` | oui | Langue de transcription Whisper. Valeurs : `"auto"` / `"fr"` / `"en"` / `"nl"`. `"auto"` active la détection automatique (Whisper `language=None`). | Non |
| `whisper_model` | `string` (enum) | `"small"` | oui | Modèle Whisper à utiliser. Valeurs : `"tiny"` / `"base"` / `"small"` / `"medium"`. Le modèle `small` est le défaut recommandé (latence < 2 s sur CPU standard, VRAM ~500 Mo). | Non |
| `models_path` | `string` (chemin) | `"~/.dictaku/models"` | oui | Répertoire local contenant les fichiers `.bin` de modèles Whisper. Chemin absolu ou relatif à `~`. | Non |
| `audio.device` | `string` | `"default"` | oui | Identifiant du périphérique d'entrée audio. `"default"` = microphone par défaut du système. Valeur alternative : nom du device tel que retourné par `cpal::available_hosts()`. | Non |
| `audio.silence_timeout_ms` | `integer` | `3000` | non | Durée de silence (en millisecondes) après laquelle la dictée s'arrête automatiquement. Plage : 500–10000. | Non |
| `audio.sample_rate` | `integer` | `16000` | oui | Taux d'échantillonnage audio en Hz. Whisper.cpp exige 16000 Hz — ne pas modifier. | Non |
| `ui.tray_animation` | `boolean` | `true` | non | Active/désactive l'animation de pulsation de l'icône tray en état Écoute. | Non |
| `ui.startup_with_windows` | `boolean` | `true` | non | Enregistre dictaku au démarrage Windows via la clé registre `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. | Non |

### Règles de validation

- `hotkey` : doit contenir au moins un modificateur + une touche alphanumérique. Les conflits avec des raccourcis système (Ctrl+Alt+Del, Win+L) sont détectés au démarrage.
- `whisper_model` : si le fichier `.bin` correspondant est absent de `models_path`, l'app démarre en mode dégradé et affiche une alerte tray.
- `audio.sample_rate` : valeur fixe, ignorée si différente de 16000 (un warning est loggé).

---

## Répertoire : `~/.dictaku/`

```
~/.dictaku/
├── config.json          # configuration utilisateur
├── models/              # fichiers .bin des modèles Whisper
│   ├── ggml-small.bin   # ~460 Mo — modèle par défaut
│   ├── ggml-tiny.bin    # ~75 Mo — option rapide, moins précis
│   └── ggml-medium.bin  # ~1.5 Go — option haute précision
└── logs/                # journaux de débogage (rotation automatique, max 7 jours)
    └── dictaku-YYYY-MM-DD.log
```

**PII :** Les fichiers de logs contiennent le texte transcrit des dictées (pour le débogage). En v0.1, les logs sont en mode verbose uniquement si `DICTAKU_DEBUG=1` est défini comme variable d'environnement. En mode normal, seuls les événements système (erreurs, changements d'état) sont loggés.

---

## Note PII globale

dictaku ne collecte aucune donnée personnelle identifiable hors de la machine locale. Le texte dicté n'est jamais transmis sur le réseau. Les logs locaux peuvent contenir des fragments de texte dicté — ils sont exclusivement locaux et l'utilisateur en a le contrôle total (suppression manuelle ou automatique par rotation).

---

## A venir — v0.2 : Historique SQLite local

Un fichier `~/.dictaku/history.db` (SQLite) sera introduit pour persister les dictées.

| Table | Champs anticipés | PII |
|---|---|---|
| `dictations` | `id`, `timestamp`, `language_detected`, `model_used`, `duration_ms`, `text`, `injected_app` | `text` = contenu dicté (PII potentiel selon contexte), `injected_app` = nom de l'app cible |
| `preferences_history` | `id`, `changed_at`, `field`, `old_value`, `new_value` | Non |

Le champ `injected_app` permettra de filtrer l'historique par application cible (ex. "toutes les dictées dans VS Code").  
La rétention par défaut sera de 30 jours, configurable.
