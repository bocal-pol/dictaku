# ADR 0003 — Injection clavier via crate `enigo`

- **Statut** : Accepté
- **Date** : 2026-08-27
- **Décideur** : vitruve-agent (architecture)

## Contexte

Dictaku doit injecter le texte transcrit dans le champ actif de n'importe quelle application (hors apps élevées UAC — hors périmètre v0.1). Le mécanisme doit être :

- portable Win/Linux/macOS (contrainte d'architecture v0.2),
- capable d'envoyer des caractères Unicode (accents FR, IJ NL),
- rapide (rate limiting pour ne pas saturer la file de messages OS).

## Options évaluées

### Option A — crate `enigo`

- API unifiée `Enigo::key_sequence(&str)` pour texte Unicode.
- Backends : Windows `SendInput` (Win32), Linux X11 via XTest / uinput, macOS `CGEventPost`.
- Version 0.2+ stable, maintenue.
- Portage sans wrapper additionnel.

### Option B — SendInput natif Win32 via `windows-rs`

- Contrôle fin (scan codes, VK codes) mais 100 % Windows.
- Impose un module OS-specific + réécriture complète pour Linux/macOS en v0.2.
- Casse la portabilité annoncée dans ADR 0001.

### Option C — Clipboard + Ctrl+V

- Colle le texte via `Ctrl+V` après avoir écrit le buffer transcrit dans le presse-papier.
- Pollue le presse-papier utilisateur (perte du contenu précédent).
- Certaines apps consomment le presse-papier différemment (formatage HTML, tableurs).
- Rejeté pour respect de l'expérience utilisateur.

### Option D — UIAutomation / Accessibility APIs

- Injection propre via l'AutomationElement du champ focus.
- Support hétérogène selon l'app (Electron ≠ Win32 ≠ WinUI).
- Complexité forte, ROI faible pour v0.1.

## Décision

Retenir **`enigo` 0.2+** (option A) avec les garde-fous suivants :

1. **Queue de sécurité** : la file d'injection refuse tout enqueue si la fenêtre focus est Dictaku elle-même (détection via `GetForegroundWindow` sur Windows, équivalent OS ailleurs) — évite les boucles infinies si l'utilisateur active accidentellement la dictée sur la fenêtre de config.
2. **Rate limiting** : bloc de N caractères + micro-pause (~ 5 ms) pour ne pas noyer la file de messages OS et laisser les apps rattraper (particulièrement Electron/JavaFX).
3. **Fallback texte** : en cas d'échec injection (fenêtre disparue, focus perdu), le dernier texte transcrit est conservé en mémoire pour être copié via un raccourci de secours ou un menu tray « Recopier la dernière dictée ».

## Conséquences

- **Positives** : portabilité gratuite, API simple, code partagé Win/Linux/macOS.
- **Négatives** : perte de contrôle bas niveau vs SendInput direct (rare dépendance de scan codes non exposée).
- **Risques** : latence d'injection sur des apps très occupées (VS Code en indexation, Chrome saturé) — mitigé par le rate limiting configurable.

## Références

- ADR 0001 (portabilité Tauri)
- `architecture/vitruve-agent/2026-08-27_architecture-report.md` (module `injection::typewriter`)
