# ADR 0001 — Choix de Tauri 2 comme framework desktop

- **Statut** : Accepté
- **Date** : 2026-08-27
- **Décideur** : vitruve-agent (architecture)
- **Contexte projet** : Dictaku v0.1 — application desktop de dictée vocale offline

## Contexte

Dictaku doit fournir :

- une petite fenêtre de configuration + tray icon Windows,
- une orchestration native performante (capture audio, appel Whisper natif via FFI, injection clavier via APIs OS),
- une UI HTML/CSS/JS (charte `dictaku_fiche.html` — Playfair Display + Inter, palette vert forêt) sans redévelopper l'esthétique en widgets natifs,
- une portabilité future vers Linux et macOS (v0.2+) sans refactor lourd.

Trois options sérieuses ont été comparées.

## Options évaluées

### Option A — Tauri 2

- Backend Rust natif, front WebView OS (WebView2 sur Windows, WebKit sur macOS, WebKitGTK sur Linux).
- Bindings natifs : bindings Rust matures pour `cpal`, `whisper-rs`, `enigo`, plugins Tauri officiels pour tray, notification, global-shortcut.
- Installer < 5 Mo à vide (les modèles Whisper sont téléchargés séparément), RAM idle typiquement 30-50 Mo.
- Sécurité : allowlist de commandes Tauri, CSP stricte sur la WebView, capabilities déclaratives (Tauri 2).
- Multi-plateforme : `cargo tauri build` cible Win/macOS/Linux avec le même code Rust.

### Option B — Electron + Node.js

- WebView Chromium embarqué (~ 120 Mo par installeur).
- Écosystème JS très riche mais bindings natifs Whisper/audio moins performants (whisper.cpp via N-API vs Rust natif).
- RAM idle 150-250 Mo, incompatible avec la NFR « tray idle < 30 Mo RAM ».
- Sécurité IPC historiquement plus laxe (contextIsolation à activer explicitement).

### Option C — WPF / WinUI 3 natif

- 100 % Windows, aucun chemin de portage vers Linux/macOS.
- Charte graphique HTML/CSS existante à recoder intégralement en XAML.
- Excellente performance mais verrouille la stack au .NET Windows.

## Décision

Retenir **Tauri 2** (option A).

## Justification

| Critère (pondération) | Tauri 2 | Electron | WPF |
|---|---|---|---|
| Empreinte installer < 10 Mo (fort) | + + + | – – | + + |
| RAM idle < 30 Mo (fort) | + + | – – | + + + |
| Réutilisation HTML/CSS existant (fort) | + + + | + + + | – – – |
| Portabilité v0.2+ (fort) | + + + | + + + | – – – |
| Performance Rust natif (fort) | + + + | + | + + |
| Sécurité par défaut (moyen) | + + + | + | + + |
| Vitesse dev v0.1 (moyen) | + + | + + + | + |

Tauri 2 est le seul candidat qui satisfait simultanément les 4 critères forts. La courbe d'apprentissage Rust est acceptée comme coût récurrent du projet.

## Conséquences

- **Positives** : installer réduit, RAM contenue, chemin de portage Linux/macOS ouvert, sécurité IPC gérée par capabilities.
- **Négatives** : dépendance à WebView2 sur Windows (déjà présent depuis Win10 21H2 ; bootstrapper embarqué pour les postes en retard). Écosystème Tauri plus jeune qu'Electron — certains plugins peuvent nécessiter du wrap manuel.
- **Risques** : compatibilité WebView2 sur postes Win10 très anciens — atténué par bootstrapper Tauri.

## Références

- `architecture/vitruve-agent/2026-08-27_architecture-report.md` (section Tech Stack)
- ADR 0003 (choix `enigo` — dépend directement de la portabilité annoncée ici)
