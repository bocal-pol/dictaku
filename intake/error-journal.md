# Error Journal — dictaku v0.1 (initialisation)

Registre des pièges anticipés avant implémentation. Chaque entrée documente un risque identifié, sa cause probable, et la mitigation recommandée.

---

## ERR-001 — Permission microphone refusée

**Sévérité :** Critique (fonctionnalité cœur inopérante)  
**Contexte :** Windows 10/11 exige une autorisation explicite pour accéder au microphone. Si l'utilisateur a refusé ou si la politique de groupe bloque l'accès, `cpal` échoue silencieusement ou retourne un stream d'erreur.

**Symptôme :** La dictée s'active (état Écoute) mais ne transcrit rien. Le buffer reste vide.

**Mitigation :**
- Vérifier l'accès au micro au démarrage via `cpal::available_input_devices()` — si aucun device, afficher alerte tray immédiatement.
- Déclarer `microphone` dans le manifeste Windows (`app.manifest`) pour déclencher la demande de permission au premier lancement.
- Message d'erreur : "Microphone inaccessible — Vérifier Paramètres Windows > Confidentialité > Microphone".
- Logger le code d'erreur WASAPI pour le debugging.

---

## ERR-002 — Conflit de raccourci global (hotkey déjà pris)

**Sévérité :** Haute (l'app ne peut pas s'activer)  
**Contexte :** `RegisterHotKey` retourne FALSE si Ctrl+Alt+D est déjà enregistré par Teams, OBS, un outil de capture, ou un autre logiciel de productivité. L'échec est silencieux par défaut.

**Symptôme :** L'app démarre normalement, le raccourci ne fonctionne pas, aucune erreur visible.

**Mitigation :**
- Capturer le résultat de `register()` dans `tauri-plugin-global-shortcut` et traiter l'erreur.
- Si conflit : afficher une notification tray "Raccourci Ctrl+Alt+D déjà utilisé — modifier dans Paramètres".
- Alternative à terme (F13) : interface de reconfiguration du hotkey.
- Note : `SetWindowsHookEx(WH_KEYBOARD_LL)` est une alternative plus permissive mais qui nécessite une boucle de messages dédiée.

---

## ERR-003 — Injection bloquée dans les applications élevées (UAC)

**Sévérité :** Haute (limitation structurelle Windows)  
**Contexte :** `SendInput` et `enigo` ne peuvent pas injecter des événements clavier dans une fenêtre dont le niveau d'intégrité (integrity level) est supérieur à celui du processus dictaku. Les apps concernées : Task Manager, éditeurs de registre, terminaux en mode admin, certains jeux.

**Symptôme :** La dictée transcrit correctement, mais aucun texte n'apparaît dans l'app cible. Pas d'erreur — `SendInput` retourne SUCCESS mais les événements sont ignorés.

**Mitigation v0.1 :**
- Documenter clairement dans le README : "Les applications lancées en tant qu'Administrateur ne sont pas supportées en v0.1".
- Détecter le cas : comparer le niveau d'intégrité du processus en focus avec celui de dictaku via `GetTokenInformation(TokenIntegrityLevel)`.
- Si détecté : afficher alerte tray "Application cible en mode Administrateur — injection non disponible".

**Mitigation v0.2 potentielle :**
- Service Windows séparé tournant en SYSTEM avec injection via session 0 isolation workaround — complexité élevée, sécurité à évaluer.

---

## ERR-004 — Modèle Whisper manquant ou corrompu

**Sévérité :** Haute (fonctionnalité cœur inopérante)  
**Contexte :** Les fichiers `.bin` Whisper ne sont pas distribués avec l'app (trop volumineux — `small` = ~460 Mo). L'utilisateur doit les placer manuellement dans `~/.dictaku/models/` ou les télécharger via l'assistant (F14).

**Symptôme :** `WhisperContext::new()` panique ou retourne une erreur au démarrage si le fichier est absent ou tronqué.

**Mitigation :**
- Vérifier l'existence et la taille minimale du fichier `.bin` au démarrage avant de tenter le chargement.
- Si manquant : mode dégradé — tray icon en état d'alerte, notification "Modèle Whisper absent — voir les instructions de téléchargement".
- Si corrompu (taille incorrecte ou hash SHA256 invalide) : idem + suggestion de re-téléchargement.
- Fournir un script PowerShell `scripts/download-model.ps1` qui télécharge et place le `.bin` automatiquement.

---

## ERR-005 — Latence trop élevée sur CPU faible (> 2 s cible non atteinte)

**Sévérité :** Moyenne (dégradation de l'expérience)  
**Contexte :** Le modèle `small` Whisper sur CPU pur prend ~1.5 s pour 5 s d'audio sur un i7 récent. Sur un CPU plus ancien (i5 6th gen, Atom) ou une VM, la latence peut dépasser 4–5 s.

**Facteurs aggravants :** modèle `medium` (x3 de latence), audio long (> 15 s), threads CPU limités.

**Mitigation :**
- Exposer le choix du modèle dans les paramètres (F11) avec indication de latence estimée.
- Documenter les configurations recommandées : `tiny` pour CPUs < 4 cœurs, `small` comme défaut, `medium` pour GPU CUDA uniquement.
- Mesurer et logger la durée de transcription pour chaque dictée en mode debug.
- Envisager le découpage de l'audio en segments courts (< 10 s) pour réduire la latence perçue.
- Note v0.3 : GPU via CUDA (whisper.cpp supporte `--gpu`) réduirait la latence à < 0.5 s.

---

## ERR-006 — Consommation mémoire excessive avec le modèle medium

**Sévérité :** Moyenne (impact stabilité système)  
**Contexte :** Le modèle `medium` requiert ~1.5 Go de RAM. Sur une machine avec 4–8 Go, cela peut provoquer une pression mémoire significative si d'autres apps lourdes tournent (Chrome, IDE, VMs).

**Mitigation :**
- Documenter clairement les prérequis mémoire par modèle dans le README et l'UI de sélection :
  - tiny: ~200 Mo RAM
  - base: ~300 Mo RAM
  - small: ~500 Mo RAM
  - medium: ~1.5 Go RAM
- Le modèle n'est chargé qu'une seule fois au démarrage et maintenu en mémoire — pas de rechargement par dictée.
- Permettre de décharger le modèle manuellement (menu tray "Libérer la mémoire") et recharger à la demande.

---

## ERR-007 — Conflit WASAPI — microphone en mode exclusif

**Sévérité :** Moyenne  
**Contexte :** Certaines apps (Discord, Teams, logiciels audio pros) prennent le microphone en mode exclusif WASAPI. Dans ce cas, `cpal` ne peut pas ouvrir un stream sur le même device.

**Symptôme :** `cpal::Stream::build_input_stream()` retourne une erreur de type `DeviceBusy`.

**Mitigation :**
- Forcer le mode partagé WASAPI dans `cpal` (option `StreamConfig` avec `share_mode: ShareMode::Shared`).
- Si toujours impossible : message d'erreur "Microphone utilisé en exclusivité par une autre application — fermer Teams/Discord d'abord".
- Logger le nom du device en conflit si récupérable.

---

## ERR-008 — Crash au démarrage si whisper.cpp n'est pas compilé

**Sévérité :** Haute (bloque l'installation)  
**Contexte :** whisper.cpp est une bibliothèque C++ qui doit être compilée. Si MSVC Build Tools ou clang ne sont pas présents sur la machine de build, le `cargo build` échoue avec des erreurs cryptiques.

**Mitigation :**
- Fournir un binaire précompilé `whisper.dll` + `whisper.lib` dans `src-tauri/resources/` pour le cas Windows.
- Documenter les prérequis de build dans le README : "MSVC Build Tools 2022 requis" avec lien d'installation.
- CI/CD : builder sur `windows-latest` GitHub Actions avec MSVC préinstallé.
- Alternative : utiliser la feature `static` de `whisper-rs` pour lier statiquement et éviter la DLL.

---

## Suivi des statuts

| ID | Statut | Version cible |
|---|---|---|
| ERR-001 | Anticipé — non implémenté | v0.1 |
| ERR-002 | Anticipé — non implémenté | v0.1 |
| ERR-003 | Anticipé — documenté, mitigation partielle | v0.1 (complète v0.2) |
| ERR-004 | Anticipé — non implémenté | v0.1 |
| ERR-005 | Anticipé — mitigation UI partielle | v0.1 (GPU v0.3) |
| ERR-006 | Anticipé — documentation seulement | v0.1 |
| ERR-007 | Anticipé — non implémenté | v0.1 |
| ERR-008 | Anticipé — binaire fallback à préparer | v0.1 |
