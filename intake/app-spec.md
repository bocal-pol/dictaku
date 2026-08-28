# App Spec — dictaku v0.1

## Quoi / Pour qui / Pourquoi

**dictaku** est une application desktop Windows (tray) qui capture la voix de l'utilisateur et injecte le texte transcrit dans le champ actif de n'importe quelle application, sans interaction souris.

**Pour qui** : usage personnel bureautique, utilisateur solo technique (développeur, rédacteur, agent). Pas de compte, pas de sync, pas de réseau.

**Pourquoi** : les solutions de dictée existantes (Dragon, Windows Speech Recognition) sont soit payantes, soit liées au cloud, soit limitées à des apps spécifiques. dictaku est 100 % offline, privacy-first, open source.

---

## Contraintes non négociables

| Contrainte | Valeur |
|---|---|
| Offline total | Aucune donnée ne quitte la machine, jamais |
| Latence cible | < 2 s entre fin de parole et injection du texte (modèle `small` sur CPU moyen) |
| Langues | FR / EN / NL avec auto-détection Whisper |
| Consommation au repos | Tray icon uniquement — 0 CPU, < 30 Mo RAM |
| Élévation UAC | L'injection dans les apps élevées (UAC) est un cas connu hors périmètre v0.1 |
| Distribution | Open source MIT/Apache 2.0, GitHub public, aucune télémétrie |

---

## Flows critiques — User Stories

### Flow 1 — Activation de la dictée

**En tant qu'utilisateur**, je veux appuyer sur `Ctrl+Alt+D` depuis n'importe quelle application pour activer le microphone, **afin de** commencer à dicter sans quitter mon contexte de travail.

**Critères d'acceptation :**

```
Given  l'app dictaku tourne en tray (état : Veille)
  And  un champ de texte est actif dans n'importe quelle app Windows
When   j'appuie sur Ctrl+Alt+D
Then   le microphone s'active (état : Écoute)
  And  l'icône tray change visuellement (vert jade plein / animation pulsante)
  And  aucune fenêtre dictaku ne prend le focus
```

---

### Flow 2 — Dictée et transcription

**En tant qu'utilisateur**, je veux parler naturellement après activation, **afin que** ma voix soit transcrite en texte en temps réel par Whisper local.

**Critères d'acceptation :**

```
Given  dictaku est en état Écoute
  And  le modèle Whisper sélectionné est chargé en mémoire
When   je parle pendant 2 à 30 secondes
Then   l'audio est capturé via le microphone par défaut Windows
  And  Whisper traite par segments (VAD — détection d'activité vocale)
  And  la transcription est accumulée en mémoire tampon
```

```
Given  le modèle Whisper n'est pas encore chargé (premier démarrage)
When   j'active la dictée
Then   un indicateur de chargement apparaît (< 3 s pour modèle `small`)
  And  la capture commence dès que le modèle est prêt
```

---

### Flow 3 — Arrêt et injection

**En tant qu'utilisateur**, je veux appuyer à nouveau sur `Ctrl+Alt+D` pour arrêter la dictée et injecter le texte transcrit dans le champ actif, **afin de** reprendre immédiatement mon travail.

**Critères d'acceptation :**

```
Given  dictaku est en état Écoute
When   j'appuie à nouveau sur Ctrl+Alt+D (ou après 3 s de silence — configurable)
Then   la capture audio s'arrête
  And  Whisper finalise la transcription du buffer restant
  And  le texte transcrit est injecté via SendInput dans le champ actif
  And  l'icône tray passe brièvement en état "Texte inséré" (vert foncé + check)
  And  l'app retourne en état Veille dans les 2 s
```

```
Given  la transcription produit du texte
When   le texte est injecté
Then   la ponctuation naturelle est respectée (Whisper gère la ponctuation)
  And  les retours à la ligne dictés ("à la ligne") sont convertis en \n
```

---

### Flow 4 — Changement de langue

**En tant qu'utilisateur**, je veux changer la langue de transcription depuis l'icône tray, **afin de** dicter dans une langue différente sans redémarrer l'app.

**Critères d'acceptation :**

```
Given  l'app est en état Veille
When   je clique droit sur l'icône tray > "Langue"
Then   un sous-menu affiche : Auto-détect / Français / English / Nederlands
  And  la sélection est persistée dans ~/.dictaku/config.json
  And  le changement est actif dès la prochaine dictée (sans rechargement du modèle)
```

```
Given  le mode "Auto-détect" est actif
When   je dicte
Then   Whisper détecte automatiquement la langue (paramètre language=None)
  And  la langue détectée est loguée dans le journal interne (debug uniquement)
```

---

## Hors périmètre v0.1 (explicite)

- Historique persistant des dictées
- Correction inline avant injection
- Sync cloud / compte utilisateur
- Support Linux / macOS
- Injection dans les applications élevées UAC (Explorer admin, Task Manager)
- Transcription en streaming continu (le texte n'est injecté qu'à l'arrêt)
