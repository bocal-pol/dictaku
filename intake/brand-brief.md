# Brand Brief — dictaku

## Positionnement

dictaku est **le scribe discret du bureau** — un outil de productivité qui s'efface derrière l'usage. Pas un assistant vocal qui parle, pas une app qui prend l'écran. Une présence silencieuse, toujours prête, qui transforme la voix en texte avec la précision d'un commis de confiance.

**Tagline principale :** La voix comme stylo  
**Tagline étendue :** Le scribe discret du bureau

---

## Identité visuelle

### Palette de couleurs

| Rôle | Nom | Hex | Usage |
|---|---|---|---|
| Fond principal / héro | Nuit bureau | `#081408` | Arrière-plan sombre, icône app |
| Fond intermédiaire | Mousse | `#143020` | Dégradés, états hover sombres |
| Couleur structurante | Forêt | `#2a6a3a` | Bordures accent, socle icône, liens actifs |
| Couleur primaire / interactive | Jade | `#4a9a5a` | Icônes, boutons, indicateurs d'état, ondes micro |
| Fond clair / surface | Brume | `#e0f0e4` | Texte sur fond sombre, surfaces légères |
| Fond page | Menthe pâle | `#f0f7f2` | Background app/page |
| Texte principal | — | `#081408` | Corps de texte sur fond clair |
| Texte secondaire | — | `#7a9a80` | Labels, métadonnées, aide contextuelle |
| Texte accent | — | `#2a4a30` | Corps sur fond blanc, descriptions |

**Règle chromatique :** jamais de rouge ou d'orange dans l'UI (ces couleurs signalent une erreur ou une alerte). Les états d'erreur utilisent un ambre neutre (`#a06a20`) pour rester dans la famille naturelle.

---

### Typographie

| Usage | Police | Graisses | Notes |
|---|---|---|---|
| Display / titres | Playfair Display | 400, 500, 400 italic | Serif élégant, lettres serrées (letter-spacing: -1px) |
| Interface / corps | Inter | 300, 400, 500 | Sans-serif lisible à toutes tailles |
| Code / raccourcis | Monospace système | 500 | Pour afficher les touches clavier (Ctrl, Alt, D) |

---

### Icône application

Micro épuré sur fond `#081408`, trait `#4a9a5a` (1.5px), socle ancré (`#2a6a3a`).  
Trois ondes sortantes sur le côté gauche avec opacité décroissante (50% / 35% / 20%) symbolisant la voix active.  
Rayon de coin : 13px (style desktop Windows moderne).

**SVG de référence (34×34, fond transparent) :**
```svg
<rect x="12" y="4" width="10" height="16" rx="5" stroke="#4a9a5a" stroke-width="1.5" fill="none"/>
<path d="M7 18 Q7 26 17 26 Q27 26 27 18" stroke="#4a9a5a" stroke-width="1.5" fill="none" stroke-linecap="round"/>
<line x1="17" y1="26" x2="17" y2="30" stroke="#2a6a3a" stroke-width="1.5" stroke-linecap="round"/>
<line x1="13" y1="30" x2="21" y2="30" stroke="#2a6a3a" stroke-width="1.5" stroke-linecap="round"/>
<line x1="10" y1="14" x2="7" y2="14" stroke="#4a9a5a" stroke-width="1" stroke-linecap="round" opacity="0.5"/>
<line x1="10" y1="17" x2="6" y2="17" stroke="#4a9a5a" stroke-width="1" stroke-linecap="round" opacity="0.35"/>
<line x1="10" y1="11" x2="6" y2="11" stroke="#4a9a5a" stroke-width="1" stroke-linecap="round" opacity="0.2"/>
```

---

## États visuels de l'icône tray

| État | Indicateur | Détail |
|---|---|---|
| Veille | Cercle `#4a9a5a` opacity 40%, bordure opacity 20% | Discret, quasi invisible |
| Écoute | Cercle `#4a9a5a` plein + animation pulse | Visible, actif |
| Texte inséré | Check `#2a6a3a` + fond opacity 10% | Confirmation, 2 s puis retour Veille |
| Erreur / micro absent | Icône micro barré, ambre `#a06a20` | Alerte non bloquante |

---

## Ton éditorial

**Trois mots clés :** Élégant · Discret · Précis

- **Élégant** : les messages sont courts, jamais verbeux. Pas de "Veuillez patienter pendant le chargement du modèle de transcription". Mais "Chargement en cours…".
- **Discret** : l'app ne réclame pas l'attention. Les notifications sont visuelles (tray), pas sonores. Aucune popup de bienvenue.
- **Précis** : les erreurs décrivent ce qui a échoué, pourquoi, et ce qu'on peut faire — en une ligne.

**Exemples de libellés corrects :**

| Situation | Libellé |
|---|---|
| Dictée active | Écoute… |
| Texte injecté | Texte inséré |
| Micro introuvable | Microphone non détecté — vérifier les paramètres audio |
| Modèle manquant | Modèle Whisper absent — télécharger depuis Paramètres |

---

## Étymologie (à utiliser en communication)

**Dictare** (latin) — prononcer à voix haute pour qu'on transcrive, fréquentatif de *dicere*.  
**支度 Shitaku** (japonais) — préparer, se tenir prêt à agir.  
dictaku : *se préparer à dicter* — comme les empereurs romains dictant leurs décrets à leurs scribes.
