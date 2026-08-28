# Kit d'intake — dictaku v0.1

Produit par socle-agent le 2026-08-27.  
Ces 6 documents fondateurs constituent le cadre de référence avant tout développement.  
A utiliser comme contexte d'entrée pour `/team-dev --boussole` puis `sdd-specify`.

---

## Index des artefacts

| Fichier | Description | Statut |
|---|---|---|
| `app-spec.md` | Spec fonctionnelle : quoi/pour qui/pourquoi, 4 flows Given/When/Then, contraintes | Complet |
| `brand-brief.md` | Identité visuelle : palette 5 tons, typographie, ton éditorial, icône SVG, états tray | Complet |
| `data-dictionary.md` | Modèle de données v0.1 (config.json) + anticipation v0.2 (historique SQLite) | Complet |
| `feature-backlog.md` | Backlog MoSCoW + RICE pour v0.1, roadmap v0.2 et v0.3 | Complet |
| `integrations.md` | Catalogue technique : whisper.cpp, Win32, enigo, cpal, Tauri v2 | Complet |
| `error-journal.md` | 8 pièges anticipés avec sévérité, symptôme et mitigation | Complet |

---

## Decisions verrouillees (ne pas remettre en question)

- Stack : Tauri 2.x (Rust + WebView HTML/CSS/JS)
- Moteur STT : whisper.cpp — 100% offline, aucun cloud
- Langues : FR / EN / NL avec auto-détection
- MVP : Windows uniquement
- Distribution : open source MIT/Apache 2.0, GitHub public, aucune télémétrie
- UI : palette vert forêt (#081408 / #2a6a3a / #4a9a5a), Playfair Display + Inter

---

## Zones A DETERMINER avant de lancer le développement

| Zone | Question ouverte | Impact |
|---|---|---|
| Licence | MIT ou Apache 2.0 ? Apache 2.0 offre une protection brevets supplémentaire. | README, `Cargo.toml`, header fichiers |
| Distribution modèles | Les modèles Whisper sont-ils téléchargés par un script (`download-model.ps1`) ou via un wizard in-app (F14) ? | Complexité onboarding v0.1 vs v0.2 |
| Stratégie VAD | Utiliser le VAD intégré de whisper.cpp (silero-vad) ou un seuil RMS maison pour l'auto-stop ? | Qualité de la détection de fin de parole |
| Build whisper.cpp | Submodule Git (build from source) ou binaire précompilé dans `resources/` ? | Complexité CI, taille du repo |
| Raccourci configurable v0.1 | F13 est classé "Could" — le promouvoir en "Should" si les conflits de hotkey sont fréquents lors des tests ? | Effort +2j, UX significativement améliorée |
| Nom du repo GitHub | `dictaku` ou `dictaku` ? (les deux orthographes coexistent dans le contexte fourni) | Cohérence branding, URL permanente |

---

## Prochaines etapes

1. Trancher les zones "A DETERMINER" ci-dessus (session cadrage avec le porteur de projet)
2. Lancer `/team-dev --boussole` avec ce kit comme contexte d'entrée
3. `sdd-specify` produira le PRD et les specs fonctionnelles détaillées
4. `sdd-plan` établira le plan de sprint v0.1 (estimation : 3–4 sprints de 1 semaine)
5. Initialiser le repo Tauri : `cargo create-tauri-app dictaku --template vanilla`
6. Placer `CLAUDE.md` à la racine du repo avant le premier commit
