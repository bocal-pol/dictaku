# Feature Backlog — dictaku

Priorités : **MoSCoW** — Must / Should / Could / Won't (v0.1)  
Score RICE = Reach × Impact × Confidence / Effort (1–10 par axe)

---

## v0.1 — MVP Windows

### Must Have

| ID | Feature | Description | RICE | Notes |
|---|---|---|---|---|
| F01 | Raccourci global Ctrl+Alt+D | Activation/désactivation de la dictée depuis n'importe quelle app via `tauri-plugin-global-shortcut` | 90 | Fonctionnalité cœur. Sans elle, rien ne fonctionne. |
| F02 | Capture audio microphone | Capture flux audio depuis le micro par défaut via `cpal` (16 kHz, mono, f32) | 90 | Dépendance de F03. |
| F03 | Transcription Whisper local | Traitement du flux audio par whisper.cpp (modèle `small`, offline, VAD intégré) | 88 | Latence cible < 2 s. |
| F04 | Injection texte dans app active | `SendInput` Win32 pour injecter les caractères dans le champ actif de la fenêtre en focus | 85 | Limitation connue : apps élevées UAC. |
| F05 | Tray icon + états visuels | Icône système avec 3 états (Veille / Écoute / Texte inséré), changement visuel par état | 80 | UX essentielle — seule interaction visuelle de l'app. |
| F06 | Config JSON locale | Lecture/écriture de `~/.dictaku/config.json` au démarrage, persistance des préférences | 78 | Sans config, l'app redémarre toujours en mode par défaut. |
| F07 | Chargement modèle Whisper | Détection et chargement du fichier `.bin` depuis `models_path` au démarrage | 75 | Gestion erreur si modèle absent (F14). |
| F08 | Sélection de langue (tray) | Menu contextuel tray : Auto / FR / EN / NL, persisté dans config | 72 | |

### Should Have

| ID | Feature | Description | RICE | Notes |
|---|---|---|---|---|
| F09 | Auto-stop sur silence | Arrêt automatique de la dictée après N ms de silence (défaut 3 s, configurable) | 65 | Réduit les dictées oubliées. VAD whisper.cpp ou seuil d'énergie RMS. |
| F10 | Démarrage automatique Windows | Enregistrement dans `HKCU\...\Run` pour lancer l'app au login | 60 | Activé par défaut, désactivable dans config. |
| F11 | Sélection du modèle Whisper | Menu tray : Tiny / Base / Small / Medium, avec rechargement à chaud | 58 | Trade-off vitesse / précision selon machine. |
| F12 | Sélection du microphone | Menu tray ou config : liste des devices audio disponibles via `cpal` | 55 | Utile quand plusieurs micros présents (casque + intégré). |

### Could Have

| ID | Feature | Description | RICE | Notes |
|---|---|---|---|---|
| F13 | Raccourci configurable | Permettre de changer le hotkey depuis les paramètres (remplace Ctrl+Alt+D) | 40 | Résout les conflits avec d'autres apps. |
| F14 | Assistant de téléchargement modèle | UI minimale pour télécharger un modèle Whisper si absent (depuis Hugging Face) | 38 | Sans cela, l'utilisateur doit placer le `.bin` manuellement. |
| F15 | Notification d'erreur micro | Alerte tray si le microphone est inaccessible ou si les permissions sont refusées | 35 | |

### Won't Have (v0.1)

| ID | Feature | Raison d'exclusion |
|---|---|---|
| F20 | Historique persistant | Complexité SQLite + UI — reporté v0.2 |
| F21 | Correction inline avant injection | Nécessite une fenêtre popup flottante — reporté v0.2 |
| F22 | Injection dans apps UAC élevées | Requiert un service Windows en tant qu'administrateur — hors périmètre |
| F23 | Support Linux / macOS | Hotkey global et injection via API Win32 sont Windows-spécifiques |
| F24 | Compte utilisateur / sync cloud | Contraire au principe offline-first |
| F25 | Télémétrie / analytics | Contraire au principe privacy-first |

---

## v0.2 — Productivité avancée (estimé ~3 mois après v0.1)

| ID | Feature | Priorité | Description |
|---|---|---|---|
| F30 | Historique local SQLite | Must | `~/.dictaku/history.db` — liste des dictées horodatées avec texte et app cible |
| F31 | UI panneau historique | Must | Fenêtre Tauri WebView pour consulter et copier les dictées précédentes |
| F32 | Correction inline (popup) | Should | Fenêtre flottante borderless sur Écoute — affiche le texte en direct et permet d'éditer avant injection |
| F33 | Commandes vocales | Could | Mots-clés réservés : "effacer", "à la ligne", "annuler" interprétés comme commandes |
| F34 | Recherche dans l'historique | Could | Filtre texte sur l'historique, filtre par app cible |

---

## v0.3 — Cross-platform + optionnel cloud (estimé ~6 mois après v0.1)

| ID | Feature | Priorité | Description |
|---|---|---|---|
| F40 | Support macOS | Must | Remplacement de l'API Win32 par les équivalents macOS (CGEventPost, NSWorkspace) |
| F41 | Support Linux (X11/Wayland) | Should | Via `xdotool` / `ydotool` selon le serveur d'affichage |
| F42 | GPU acceleration (CUDA/Metal) | Should | whisper.cpp avec backend CUDA pour latence < 0.5 s sur GPU |
| F43 | Cloud STT optionnel | Could | Mode alternatif : OpenAI Whisper API ou Azure Speech — opt-in explicite, aucun défaut |
| F44 | Synchronisation config multi-postes | Could | Sync du `config.json` via fichier partagé (Dropbox, OneDrive) — pas de compte dictaku |

---

## v0.4 — Clavier mobile (estimé ~12 mois après v0.1)

> **Concept** : Dictaku Mobile = clavier système (IME) remplaçant Gboard/SwiftKey avec Whisper local embarqué.
> Différenciateur unique sur le marché : **100% offline, aucune donnée vocale envoyée à un tiers**.
> Aucun clavier mobile existant (Gboard, SwiftKey, Clavier iOS) ne propose Whisper local.

### Stack mobile recommandée

| Composant | Choix | Raison |
|---|---|---|
| Framework | **Flutter** (pas Tauri) | Tauri ne supporte pas les IME natifs Android/iOS — Flutter oui via plugins |
| STT | whisper.cpp via FFI Flutter (plugin `whisper_flutter`) | Même moteur que le desktop, cohérence de qualité |
| Clavier Android | `InputMethodService` (IME natif Android) | Seule voie pour s'enregistrer comme clavier système |
| Clavier iOS | `Custom Keyboard Extension` (UIKit) | Restrictions Apple : pas d'accès micro depuis l'extension — contournement via app principale |

### Features v0.4

| ID | Feature | Priorité | Description |
|---|---|---|---|
| F50 | Clavier Android IME | Must | S'enregistre comme méthode de saisie système — remplace Gboard dans n'importe quelle app Android |
| F51 | Bouton micro dans le clavier | Must | Bouton discret dans la barre du clavier → appui → dictée → texte inséré dans le champ actif |
| F52 | Whisper local Android | Must | Modèle `tiny` (39 Mo) embarqué — inférence sur CPU ARM via whisper.cpp NDK |
| F53 | FR / EN / NL auto-détection | Must | Même logique que le desktop |
| F54 | Clavier AZERTY/QWERTY classique | Should | Layout clavier complet en fallback quand pas de dictée |
| F55 | iOS Custom Keyboard Extension | Should | Support iOS via extension clavier — limitations Apple sur accès micro à évaluer |
| F56 | Modèle Whisper téléchargeable | Should | Choix tiny/base depuis les paramètres — stocké dans app sandbox |
| F57 | Historique dictées mobile | Could | Synchronisable avec l'historique desktop via fichier partagé (OneDrive/Dropbox) |
| F58 | Mode hors-clavier (widget) | Could | Widget Android qui écoute et injecte dans l'app active via AccessibilityService |

### Contraintes techniques connues

- **Android** : `InputMethodService` bloque l'accès micro en arrière-plan sur Android 12+ → solution : pipeline audio dans le service IME lui-même
- **iOS** : Apple interdit l'accès micro depuis une Custom Keyboard Extension — la dictée passe par l'app principale avec un handoff vers l'extension
- **Taille modèle** : `tiny` (39 Mo) recommandé pour le mobile, `base` (74 Mo) en option pour les appareils > 4 Go RAM
- **Batterie** : inférence whisper.cpp sur CPU ARM ≈ 200-400 mAh / heure d'écoute active — acceptable en usage ponctuel

### Prérequis avant de démarrer v0.4

- [ ] v0.1 desktop validé et distribué
- [ ] Évaluation de `whisper_flutter` (plugin FFI whisper.cpp pour Flutter)
- [ ] Test de performance whisper.cpp sur ARM (Snapdragon 8 Gen 2 cible)
- [ ] Décision architecture : app Flutter standalone ou monorepo avec le desktop Tauri
