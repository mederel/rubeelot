// Lightweight client-side localisation. English source text is the key; a missing entry falls back to English.
// Each non-English language is a "locale" object; French lives here and further locales register themselves
// in window.beelotLocales (see i18n-nl.js, which must load first).
// French is the default; a language chosen in Settings takes precedence.
const languageKey = "beelot.language";
const supportedLanguages = ["en", "fr", "nl"];

function detectLanguage() {
  try {
    const stored = window.localStorage.getItem(languageKey);
    if (supportedLanguages.includes(stored)) return stored;
  } catch (_) { /* storage unavailable */ }
  return "fr";
}

const language = detectLanguage();

const frenchSuits = { clubs: "trèfle", diamonds: "carreau", hearts: "cœur", spades: "pique",
  "all trumps": "tout atout", "no trumps": "sans atout" };
const frenchRankNames = { A: "as", K: "roi", Q: "dame", J: "valet" };
const frenchRankLabels = { A: "A", K: "R", Q: "D", J: "V" };

const fr = {
  // Document
  "Beelot — Play Belote": "Beelot — Jouer à la belote",
  "Start a game of Belote.": "Lancez une partie de belote.",
  "Beelot home": "Accueil Beelot",
  "Settings": "Paramètres",
  "Game highlights": "Points forts du jeu",
  "Game modes": "Modes de jeu",

  // Home
  "Belote, remixed ✨": "La belote, en version remix ✨",
  "Deal in.": "À la donne.",
  "Good times": "Les bons moments",
  "await.": "vous attendent.",
  "Quick games, clever plays and big table energy. Challenge our bots or bring your favourite people.":
    "Parties rapides, coups malins et grosse ambiance de tablée. Défiez nos bots ou invitez vos proches.",
  "⚡ Jump in instantly": "⚡ Jouez tout de suite",
  "♣ Classic & Contrée": "♣ Classique et Contrée",
  "♥ Better with friends": "♥ Meilleur entre amis",
  "Quick game": "Partie rapide",
  "Grab a seat and take on three lively bots.": "Prenez place face à trois bots pleins d'entrain.",
  "Bring the crew": "Réunissez la bande",
  "Open a private table and invite your favourite rivals.": "Ouvrez une table privée et invitez vos rivaux préférés.",
  "Learn the moves": "Apprenez les bases",
  "Master trump and tricks without the pressure.": "Maîtrisez l'atout et les plis sans pression.",
  "Table talk": "Parlons règles",
  "Everything you need to settle a rules debate.": "Tout pour trancher un débat sur les règles.",
  "Shuffle up, team up, and race to 1,000 points.": "Battez les cartes, faites équipe et visez les 1 000 points.",

  // Bot setup
  "Your table is ready ⚡": "Votre table est prête ⚡",
  "Pick your vibe.": "Choisissez votre style.",
  "Choose your flavour of Belote and how spicy you want the competition.":
    "Choisissez votre belote et le niveau de piquant de l'adversaire.",
  "Game variant": "Variante",
  "Classic Belote": "Belote classique",
  "Contrée": "Contrée",
  "Traditional upcard bidding.": "Enchères traditionnelles sur la carte retournée.",
  "Contract auction with coinche.": "Enchères de contrat avec coinche.",
  "Bot difficulty": "Difficulté des bots",
  "Relaxed": "Détendu",
  "A gentler game to learn and practise.": "Une partie plus douce pour apprendre et s'entraîner.",
  "Challenging": "Exigeant",
  "More demanding opponents for experienced players.": "Des adversaires plus coriaces pour les joueurs expérimentés.",
  "Deal the cards": "Distribuer les cartes",
  "Back to game modes": "Retour aux modes de jeu",

  // Bot game
  "Bot card room": "Salle de cartes des bots",
  "Bots ·": "Bots ·",
  "Team scores": "Scores des équipes",
  "North–South": "Nord–Sud",
  "East–West": "Est–Ouest",
  "VS": "VS",
  "Game status": "État de la partie",
  "Trump": "Atout",
  "Contract": "Contrat",
  "Turn": "Tour",
  "Four-player card table": "Table de cartes à quatre joueurs",
  "Trick": "Pli",
  "Play the opening card": "Jouez la première carte",
  "Your eight cards": "Vos huit cartes",
  "Your five cards": "Vos cinq cartes",
  "Deal next round": "Donner la manche suivante",
  "Rematch": "Revanche",
  "Rules reference": "Rappel des règles",
  "Leave table": "Quitter la table",

  // Bidding
  "Choose trump": "Choisir l'atout",
  "Make your call.": "Faites votre annonce.",
  "Bidding table with four players": "Table d'enchères à quatre joueurs",
  "Upturned card": "Carte retournée",
  "Your five-card hand": "Votre main de cinq cartes",
  "Your eight-card hand": "Votre main de huit cartes",
  "Accept upturned suit": "Prendre à la couleur retournée",
  "Pass": "Passer",
  "Choose a trump suit": "Choisissez une couleur d'atout",
  "Clubs": "Trèfle",
  "Diamonds": "Carreau",
  "Hearts": "Cœur",
  "Spades": "Pique",
  "No contract yet": "Pas encore de contrat",
  "Contract value": "Valeur du contrat",
  "Trump suit": "Couleur d'atout",
  "Make bid": "Annoncer",
  "Coinche": "Coinche",
  "Coinche!": "Coinche !",
  "{0} passes.": "{0} passe.",
  "{0} takes with {1}.": "{0} prend à {1}.",
  "Contrée auction": "Enchères de Contrée",
  "Current contract: {0} {1} by {2}": "Contrat actuel : {0} {1} par {2}",

  // Public matchmaking
  "Sign in to keep track of your matches.": "Connectez-vous pour garder la trace de vos parties.",
  "Sign-in did not complete. Please try again.": "La connexion n’a pas abouti. Veuillez réessayer.",
  "Sign in with a passkey": "Se connecter avec une clé d’accès",
  "Create an account": "Créer un compte",
  "Your passkey is the only way to sign in. If you lose it, you lose your account: there is no recovery.": "Votre clé d’accès est le seul moyen de vous connecter. Si vous la perdez, vous perdez votre compte : il n’y a aucune récupération possible.",
  "I understand, create my passkey": "J’ai compris, créer ma clé d’accès",
  "Your browser does not support passkeys. You can still play as a guest.": "Votre navigateur ne prend pas en charge les clés d’accès. Vous pouvez toujours jouer en invité.",
  "Too many attempts. Please try again later.": "Trop de tentatives. Veuillez réessayer plus tard.",
  "Signed in as {0}": "Connecté en tant que {0}",
  "Last sign-in: {0}": "Dernière connexion : {0}",
  "This is your first sign-in.": "C’est votre première connexion.",
  "Sign out": "Se déconnecter",
  "Sign in": "Se connecter",
  "Reactions": "Réactions",
  "Send and see quick reactions at online tables.": "Envoyer et voir des réactions rapides aux tables en ligne.",
  "Well played!": "Bien joué !",
  "Nice hand!": "Belle main !",
  "Oops!": "Oups !",
  "Wow!": "Waouh !",
  "Let me think…": "Je réfléchis…",
  "Good game!": "Bonne partie !",
  "{0}: {1}": "{0} : {1}",
  "Slow down: wait a moment before reacting again.": "Doucement : attendez un instant avant de réagir à nouveau.",
  "Reactions open once the game starts.": "Les réactions s’ouvrent quand la partie commence.",
  "Choose a reaction.": "Choisissez une réaction.",
  "Pass and play": "Jouer à plusieurs",
  "Share one device and take turns with your hand.": "Partagez un appareil et jouez votre main chacun votre tour.",
  "One device, several players": "Un appareil, plusieurs joueurs",
  "Write each player's name in the seat they take. Bots take the empty seats; North and South play together against East and West.": "Écrivez le nom de chaque joueur à la place qu’il prend. Les bots prennent les places vides ; Nord et Sud jouent ensemble contre Est et Ouest.",
  "Players": "Joueurs",
  "North": "Nord",
  "East": "Est",
  "South": "Sud",
  "West": "Ouest",
  "Bot": "Bot",
  "Start pass and play": "Commencer la partie",
  "Pass the device to {0}": "Passez l’appareil à {0}",
  "Keep your cards hidden from the other players.": "Gardez vos cartes cachées des autres joueurs.",
  "I'm ready": "C’est parti",
  "Pass and play needs at least two players.": "Il faut au moins deux joueurs pour jouer à plusieurs.",
  "Each player needs a different name.": "Chaque joueur doit avoir un nom différent.",
  "Fill in the four seats.": "Remplissez les quatre places.",
  "Hint": "Indice",
  "Lead trumps": "Jouez atout",
  "Cash a master card": "Encaissez une carte maîtresse",
  "Lead a low card": "Entamez une petite carte",
  "Give points to your partner": "Donnez des points à votre partenaire",
  "Keep your partner's trick": "Laissez le pli à votre partenaire",
  "Win the trick cheaply": "Gagnez le pli au meilleur prix",
  "Throw your cheapest card": "Défaussez votre carte la plus faible",
  "The best card for the rest of the round": "La meilleure carte pour la suite de la manche",
  "Take: strong hand in {0}": "Prenez : belle main à {0}",
  "Support your partner's bid": "Soutenez l'annonce de votre partenaire",
  "Bid what your hand is worth": "Annoncez ce que vaut votre main",
  "Coinche: you expect to defeat them": "Coinchez : vous pensez les faire chuter",
  "Pass: your hand is too weak": "Passez : votre main est trop faible",
  "It is not your turn.": "Ce n'est pas votre tour.",
  "There is no hint to give now.": "Il n'y a pas d'indice à donner pour le moment.",
  "Player names are limited to 30 characters.": "Les noms de joueur sont limités à 30 caractères.",
  "My statistics": "Mes statistiques",
  "Your progress": "Votre progression",
  "Sign in to see your statistics.": "Connectez-vous pour voir vos statistiques.",
  "No finished matches yet. Finish a match while signed in and it will appear here.": "Aucune partie terminée pour l’instant. Terminez une partie en étant connecté et elle apparaîtra ici.",
  "Match record": "Bilan des parties",
  "By variant": "Par variante",
  "Solo games by difficulty": "Parties contre les bots par difficulté",
  "Contracts": "Contrats",
  "Recent matches": "Parties récentes",
  "Matches played": "Parties jouées",
  "Played": "Jouées",
  "Won": "Gagnée",
  "Lost": "Perdue",
  "Wins": "Victoires",
  "Losses": "Défaites",
  "Win rate": "Taux de victoire",
  "Against bots": "Contre les bots",
  "Contracts taken by your team": "Contrats pris par votre équipe",
  "Contracts made": "Contrats réussis",
  "Coinches by your team": "Coinches de votre équipe",
  "Coinches against your team": "Coinches contre votre équipe",
  "{0}, {1} defeated the contract": "{0}, dont {1} ont fait chuter le contrat",
  "{0}, {1} defeated your contract": "{0}, dont {1} ont fait chuter votre contrat",
  "Capots": "Capots",
  "Belotes": "Belotes",
  "Average points per round": "Points moyens par donne",
  "Date": "Date",
  "Mode": "Mode",
  "Variant": "Variante",
  "Partner": "Partenaire",
  "Opponents": "Adversaires",
  "Score": "Score",
  "Result": "Résultat",
  "Find a game": "Trouver une partie",
  "Join an open table — bots fill in if nobody shows up.": "Rejoignez une table ouverte — des bots complètent si personne ne vient.",
  "Strangers welcome ♠": "Inconnus bienvenus ♠",
  "Find a game.": "Trouvez une partie.",
  "Pick a variant and we'll seat you at an open table. If it isn't full within a minute, bots take the empty seats.":
    "Choisissez une variante et nous vous installons à une table ouverte. Si elle n'est pas complète en une minute, des bots prennent les places libres.",
  "Quick match": "Partie express",
  "Looking for players… bots take the empty seats in {0} s.": "Recherche de joueurs… des bots prennent les places libres dans {0} s.",

  // Private tables
  "Friends, rivals, legends ♥": "Amis, rivaux, légendes ♥",
  "Bring the crew.": "Réunissez la bande.",
  "Make a room in seconds or use a six-character invite to jump into the fun.":
    "Créez une salle en quelques secondes ou utilisez un code d'invitation à six caractères pour rejoindre la fête.",
  "Create a table": "Créer une table",
  "Your name": "Votre nom",
  "Player": "Joueur",
  "Create private table": "Créer une table privée",
  "Join friends": "Rejoindre des amis",
  "Invitation code": "Code d'invitation",
  "Join table": "Rejoindre la table",
  "Private table": "Table privée",
  "Public table": "Table publique",
  "Gather your team.": "Rassemblez votre équipe.",
  "Copy": "Copier",
  "Copied": "Copié",
  "Private table seats": "Places de la table privée",
  "Turn timer": "Minuteur de tour",
  "No timer": "Sans minuteur",
  "30 seconds": "30 secondes",
  "60 seconds": "60 secondes",
  "Save": "Enregistrer",
  "I am ready": "Je suis prêt",
  "Not ready": "Pas prêt",
  "Start game": "Lancer la partie",
  "Start now with bots": "Lancer maintenant avec des bots",
  "Auction": "Enchères",
  "Private game status": "État de la partie privée",
  "Declaring team": "Équipe déclarante",
  "Private team scores": "Scores des équipes (table privée)",
  "Four-player private card table": "Table de cartes privée à quatre joueurs",
  "Your cards": "Vos cartes",
  "Current trick": "Pli en cours",
  "Bid": "Annoncer",
  "Game started": "Partie commencée",
  "(you)": "(vous)",
  "You": "Vous",
  "{0} (you)": "{0} (vous)",
  "Ready": "Prêt",
  "Waiting": "En attente",
  "Bot takeover": "Repris par un bot",
  "Disconnected": "Déconnecté",
  "Connected": "Connecté",
  "Partners sit across from each other: pick an empty seat to choose yours. Four players must be ready before the table owner can start the game, or the owner can start now and bots take the empty seats.":
    "Les partenaires sont assis face à face : choisissez une place libre pour choisir le vôtre. Quatre joueurs doivent être prêts pour que le propriétaire de la table lance la partie, ou bien il peut démarrer tout de suite et les bots prennent les places libres.",
  "Empty seat": "Place libre",
  "Sit here": "S'asseoir ici",
  "Sit in this {0} seat": "S'asseoir à cette place {0}",
  "Turn timer: {0} seconds. A warning appears with 10 seconds remaining; an expired turn is played by a bot.":
    "Minuteur de tour : {0} secondes. Un avertissement apparaît à 10 secondes de la fin ; un tour expiré est joué par un bot.",
  "Turn timer is disabled.": "Le minuteur de tour est désactivé.",
  "Variant: {0}": "Variante : {0}",
  "The game has started. Calls and card play update for every player automatically.":
    "La partie a commencé. Les annonces et les cartes jouées se mettent à jour automatiquement pour tous les joueurs.",
  "That table is no longer available.": "Cette table n'est plus disponible.",
  "No upturned card in Contrée.": "Pas de carte retournée en Contrée.",
  "Something went wrong. Please try again.": "Une erreur est survenue. Veuillez réessayer.",
  "Sign in again to delete your account.": "Reconnectez-vous pour supprimer votre compte.",
  "Sign in first.": "Connectez-vous d'abord.",
  "The request is not valid.": "La requête n'est pas valide.",
  "The request is too large.": "La requête est trop volumineuse.",
  "Not found.": "Introuvable.",
  "The server is busy. Please try again later.": "Le serveur est occupé. Veuillez réessayer plus tard.",
  "Too many requests. Please slow down.": "Trop de requêtes. Veuillez ralentir.",

  // Tutorial
  "Learn Belote": "Apprendre la belote",
  "Tutorial": "Tutoriel",
  "Next lesson": "Leçon suivante",
  "Finish tutorial": "Terminer le tutoriel",
  "Replay tutorial": "Rejouer le tutoriel",
  "Lesson {0} of {1}": "Leçon {0} sur {1}",
  "Choose a card to continue.": "Choisissez une carte pour continuer.",
  "Not quite. Look at the highlighted card, then try the next lesson.":
    "Pas tout à fait. Regardez la carte en surbrillance, puis passez à la leçon suivante.",
  "Trump wins": "L'atout l'emporte",
  "Hearts are trump. Which card is strongest?": "Cœur est atout. Quelle carte est la plus forte ?",
  "Correct. At trump, the jack is the strongest card.": "Exact. À l'atout, le valet est la carte la plus forte.",
  "Normal card strength": "Force des cartes ordinaires",
  "Clubs are not trump. Which card wins this trick?": "Trèfle n'est pas atout. Quelle carte remporte ce pli ?",
  "Correct. Outside trump, ace is stronger than 10.": "Exact. Hors atout, l'as est plus fort que le 10.",
  "Follow suit": "Fournir la couleur",
  "Hearts were led. Which card must you play if these are your choices?":
    "Cœur a été demandé. Quelle carte devez-vous jouer si vous avez ce choix ?",
  "Correct. You must follow the lead suit when you can.": "Exact. Vous devez fournir la couleur demandée quand vous le pouvez.",
  "Take the trick": "Prendre le pli",
  "Spades are trump and clubs were led. Which card takes this trick?":
    "Pique est atout et trèfle a été demandé. Quelle carte prend ce pli ?",
  "Correct. A trump card beats cards in the lead suit.": "Exact. Un atout bat les cartes de la couleur demandée.",

  // Rules
  "Classic suit-trump Belote, played to 1,000 points. Tout-Atout and Sans-Atout are not part of this variant.":
    "Belote classique à atout couleur, jouée en 1 000 points. Le Tout-Atout et le Sans-Atout ne font pas partie de cette variante.",
  "Deal and trump": "Distribution et atout",
  "Each player receives five cards and one card is turned face up. First, accept that suit or pass. After all pass, choose another suit; a second all-pass redeals.":
    "Chaque joueur reçoit cinq cartes et une carte est retournée. Au premier tour, prenez cette couleur ou passez. Si tout le monde passe, choisissez une autre couleur ; un second tour où tout le monde passe entraîne une nouvelle donne.",
  "Card order": "Ordre des cartes",
  "At trump: jack, 9, ace, 10, king, queen, 8, 7. In other suits: ace, 10, king, queen, jack, 9, 8, 7.":
    "À l'atout : valet, 9, as, 10, roi, dame, 8, 7. Dans les autres couleurs : as, 10, roi, dame, valet, 9, 8, 7.",
  "Playing a card": "Jouer une carte",
  "Follow the lead suit when possible. If void and an opponent wins, trump and overtrump when able. If your partner wins, you may discard.":
    "Fournissez la couleur demandée quand c'est possible. Si vous n'en avez pas et qu'un adversaire est maître, coupez et surcoupez si vous le pouvez. Si votre partenaire est maître, vous pouvez vous défausser.",
  "Scoring": "Décompte",
  "Card points total 162, including 10 points for the last trick (Dix de der). King and queen of trump declare Belote/Rebelote for 20 points. The declaring team must score more than its opponents, counting Belote and announcements.":
    "Les cartes valent 162 points au total, dont 10 points pour le dernier pli (dix de der). Le roi et la dame d'atout annoncent Belote/Rebelote pour 20 points. L'équipe qui a pris doit marquer plus que ses adversaires, Belote et annonces comprises.",
  "Contrée variant": "Variante Contrée",
  "All eight cards are dealt before an auction from 80 to 160, in steps of 10. Each call names a trump suit and must raise the current contract. Three passes after a bid close the auction. An opponent may coinche, doubling the round score; Belote/Rebelote remains active.":
    "Les huit cartes sont distribuées avant des enchères de 80 à 160, par pas de 10. Chaque annonce désigne une couleur d'atout et doit surenchérir sur le contrat en cours. Trois passes après une annonce closent les enchères. Un adversaire peut coincher, ce qui double le score de la manche ; la Belote/Rebelote reste valable.",

  // Your data (Settings)
  "Your data": "Vos données",
  "Download a copy of what Beelot keeps about you, or delete your account.":
    "Téléchargez une copie de ce que Beelot conserve à votre sujet, ou supprimez votre compte.",
  "Download my data": "Télécharger mes données",
  "Delete my account": "Supprimer mon compte",
  "Your account, your passkey and your sign-in history will be deleted. Finished matches stay in other players' history, shown as “Deleted player”.":
    "Votre compte, votre clé d’accès et votre historique de connexion seront supprimés. Les parties terminées restent dans l'historique des autres joueurs, sous le nom « Joueur supprimé ».",
  "Type this word to confirm:": "Tapez ce mot pour confirmer :",
  "DELETE": "SUPPRIMER",
  "Delete my account for good": "Supprimer définitivement mon compte",
  "Cancel": "Annuler",
  "You can download your data again in {0} minutes.":
    "Vous pourrez de nouveau télécharger vos données dans {0} minutes.",
  "Your account has been deleted.": "Votre compte a été supprimé.",
  "For your safety, sign in again to delete your account.":
    "Par sécurité, reconnectez-vous pour supprimer votre compte.",


  // Tout-atout and sans-atout (US-063)
  "Tout-atout and sans-atout": "Tout atout et sans atout",
  "Bids may also name all trumps or no trumps.": "Les annonces peuvent aussi nommer le tout atout ou le sans atout.",
  "Only the four suits are played.": "Seules les quatre couleurs se jouent.",
  "Tout-atout and sans-atout are played.": "Le tout atout et le sans atout se jouent.",
  "All trumps": "Tout atout",
  "No trumps": "Sans atout",
  "Only the table owner can change tout-atout and sans-atout.": "Seul le propriétaire de la table peut modifier le tout atout et le sans atout.",
  "Tout-atout and sans-atout cannot change after the game starts.": "Le tout atout et le sans atout ne peuvent pas changer après le début de la partie.",
  "Public tables always play with tout-atout and sans-atout.": "Les tables publiques jouent toujours avec le tout atout et le sans atout.",
  "Tout-atout and sans-atout are not played at this table.": "Le tout atout et le sans atout ne se jouent pas à cette table.",
  "Besides the four suits, a Contrée bid can name sans-atout (no trumps) or tout-atout (all trumps) when the table plays them. A bid must be higher in value whatever the contract. Nothing is cut in either contract: follow the lead suit when you can, otherwise play any card. In tout-atout you must also beat the winning card when you can.":
    "Outre les quatre couleurs, une annonce de Contrée peut nommer le sans atout ou le tout atout quand la table les joue. Une annonce doit être supérieure en valeur, quel que soit le contrat. On ne coupe dans aucun des deux contrats : fournissez la couleur demandée quand vous le pouvez, sinon jouez la carte de votre choix. Au tout atout, vous devez aussi monter sur la carte qui tient le pli quand vous le pouvez.",
  "Sans-atout card points: Ace 19, 10 10, King 4, Queen 3, Jack 2, 9, 8 and 7 0. Order: ace, 10, king, queen, jack, 9, 8, 7. The cards total 162 with the 10 points for the last trick (Dix de der).":
    "Points des cartes au sans atout : As 19, 10 10, Roi 4, Dame 3, Valet 2, 9, 8 et 7 0. Ordre : as, 10, roi, dame, valet, 9, 8, 7. Les cartes valent 162 points au total avec les 10 points du dernier pli (dix de der).",
  "Tout-atout card points: Jack 13, 9 9, Ace 6, 10 5, King 3, Queen 2, 8 and 7 0. Order in every suit: jack, 9, ace, 10, king, queen, 8, 7. The cards total 162 with the 10 points for the last trick (Dix de der).":
    "Points des cartes au tout atout : Valet 13, 9 9, As 6, 10 5, Roi 3, Dame 2, 8 et 7 0. Ordre dans chaque couleur : valet, 9, as, 10, roi, dame, 8, 7. Les cartes valent 162 points au total avec les 10 points du dernier pli (dix de der).",
  "In tout-atout, Belote/Rebelote can be declared in every suit; sans-atout has none. Sans-atout carrés score aces 200, tens 150, and kings, queens, jacks or nines 100.":
    "Au tout atout, la Belote/Rebelote peut s’annoncer dans toutes les couleurs ; il n’y en a pas au sans atout. Les carrés du sans atout valent 200 pour les as, 150 pour les dix, et 100 pour les rois, dames, valets ou neuf.",

  // Announcements (US-060)
  "Announcements": "Annonces",
  "On": "Activées",
  "Off": "Désactivées",
  "Declare tierce, cinquante, cent and carrés.": "Annoncez tierce, cinquante, cent et carrés.",
  "Card points and Belote only.": "Seulement les points des cartes et la Belote.",
  "Declare: {0}": "Annoncer : {0}",
  "{0} declares.": "{0} annonce.",
  "{0} score {1} points of announcements.": "{0} marquent {1} points d’annonces.",
  "Announcements are on.": "Les annonces sont activées.",
  "Announcements are off.": "Les annonces sont désactivées.",
  "Match target": "Objectif de la partie",
  "500 points": "500 points",
  "1,000 points": "1 000 points",
  "1,500 points": "1 500 points",
  "2,000 points": "2 000 points",
  "A quick match.": "Une partie rapide.",
  "The usual match.": "La partie habituelle.",
  "A long match.": "Une longue partie.",
  "A very long match.": "Une très longue partie.",
  "The match is played to {0} points.": "La partie se joue en {0} points.",
  "Target": "Objectif",
  "Only the table owner can change the match target.": "Seul le propriétaire de la table peut changer l'objectif de la partie.",
  "The match target cannot change after the game starts.": "L'objectif de la partie ne peut plus changer une fois la partie commencée.",
  "Public tables always play to 1,000 points.": "Les tables publiques se jouent toujours en 1 000 points.",
  "The match target must be 500, 1,000, 1,500 or 2,000 points.": "L'objectif de la partie doit être de 500, 1 000, 1 500 ou 2 000 points.",
  "Tierce to the {0}": "Tierce, {0} en tête",
  "Cinquante to the {0}": "Cinquante, {0} en tête",
  "Cent to the {0}": "Cent, {0} en tête",
  "Carré of {0}": "Carré de {0}",
  "jacks": "valets",
  "nines": "neuf",
  "aces": "as",
  "tens": "dix",
  "kings": "rois",
  "queens": "dames",
  "Announcements: {0} {1} ({2}).": "Annonces : {0} {1} ({2}).",
  "Announcements: {0} {1} ({2}), which go to {3}.": "Annonces : {0} {1} ({2}), qui reviennent à {3}.",
  "tierce": "tierce",
  "cinquante": "cinquante",
  "cent": "cent",
  "carré": "carré",
  "Announcements: none.": "Annonces : aucune.",
  "Only the table owner can change announcements.": "Seul le propriétaire de la table peut modifier les annonces.",
  "Announcements cannot change after the game starts.":
    "Les annonces ne peuvent plus changer une fois la partie commencée.",
  "Public tables always play with announcements.": "Les tables publiques jouent toujours avec les annonces.",
  "Declare tierce (three cards in a row, 20), cinquante (four, 50), cent (five or more, 100) and carrés (jacks 200, nines 150, aces, tens, kings or queens 100) when you play your first card. Sequences follow 7, 8, 9, 10, jack, queen, king, ace in one suit; a card counts once, carrés first. Only the team with the best announcement scores, and it scores all of its own: the higher value wins, then the longer run, then the higher card, then trump, then whoever declared first. Belote and announcements count towards the contract.":
    "Annoncez tierce (trois cartes qui se suivent, 20), cinquante (quatre, 50), cent (cinq ou plus, 100) et carrés (valets 200, neuf 150, as, dix, rois ou dames 100) en jouant votre première carte. Les suites suivent 7, 8, 9, 10, valet, dame, roi, as dans une couleur ; une carte ne compte qu’une fois, les carrés d’abord. Seule l’équipe qui a la meilleure annonce marque, et elle marque toutes les siennes : la plus forte valeur l’emporte, puis la plus longue suite, puis la plus haute carte, puis l’atout, puis celui qui a annoncé le premier. La Belote et les annonces comptent pour le contrat.",

  // Privacy policy
  "How we handle your data": "Comment nous traitons vos données",
  "Privacy policy": "Politique de confidentialité",
  "This policy explains what Beelot keeps about you, why, for how long, and how to exercise your rights. Guests can play every mode without an account; their games are not recorded.":
    "Cette politique explique ce que Beelot conserve à votre sujet, pourquoi, pendant combien de temps, et comment exercer vos droits. Les invités peuvent jouer à tous les modes sans compte ; leurs parties ne sont pas enregistrées.",
  "Who is responsible": "Qui est responsable",
  "Data controller": "Responsable du traitement",
  "Privacy contact": "Contact données personnelles",
  "not configured": "non configuré",
  "Write to the privacy contact with any question about your data or to exercise your rights.":
    "Écrivez au contact données personnelles pour toute question sur vos données ou pour exercer vos droits.",
  "What we store and why": "Ce que nous conservons et pourquoi",
  "Your account: a pseudonym, when the account was created, and your last two sign-ins, which are shown to you so that you can spot a sign-in you did not make. It lets you sign in and keep your results. Legal basis: the contract between you and Beelot (GDPR article 6(1)(b)).":
    "Votre compte : un pseudonyme, la date de création du compte, et celles de vos deux dernières connexions, qui vous sont montrées pour que vous repériez une connexion que vous n’avez pas faite. Il vous permet de vous connecter et de garder vos résultats. Base légale : le contrat entre vous et Beelot (article 6, paragraphe 1, b) du RGPD).",
  "Your finished matches: the seats, the final score and every round, so that you can see your statistics. The other players at the table see your pseudonym. Legal basis: the contract.":
    "Vos parties terminées : les places, le score final et chaque manche, pour que vous puissiez consulter vos statistiques. Les autres joueurs de la table voient votre pseudonyme. Base légale : le contrat.",
  "Security events: sign-ups, sign-ins and failed sign-ins, sign-outs, expired sessions, refused requests, data downloads and account deletions, each with its time, IP address and outcome. No passkey identifier is ever part of them. They help detect and investigate abuse. Legal basis: our legitimate interest in keeping Beelot secure (GDPR article 6(1)(f)).":
    "Les événements de sécurité : inscriptions, connexions réussies et échouées, déconnexions, sessions expirées, demandes refusées, téléchargements de données et suppressions de compte, chacun avec son heure, l'adresse IP et le résultat. Aucun identifiant de clé d’accès n’y figure jamais. Ils servent à détecter les abus et à enquêter sur eux. Base légale : notre intérêt légitime à assurer la sécurité de Beelot (article 6, paragraphe 1, f) du RGPD).",
  "Your passkey: its public key, its signature counter, and when it was created and last used. Its private key never leaves your device, and Beelot never asks for your email address. Legal basis: the contract.":
    "Votre clé d’accès : sa clé publique, son compteur de signatures, et ses dates de création et de dernière utilisation. Sa clé privée ne quitte jamais votre appareil, et Beelot ne vous demande jamais votre adresse e-mail. Base légale : le contrat.",
  "Who processes it": "Qui les traite",
  "Hosting": "Hébergement",
  "Your data is never sold, shared for advertising or used to profile you.":
    "Vos données ne sont jamais vendues, partagées à des fins publicitaires ni utilisées pour vous profiler.",
  "How long we keep it": "Combien de temps nous les gardons",
  "A passkey made for a sign-up that was never finished: erased within two days.":
    "Une clé d’accès créée pour une inscription jamais terminée : effacée sous deux jours.",
  "Accounts: deleted 3 years after the last sign-in, with their passkey and security events.":
    "Comptes : supprimés 3 ans après la dernière connexion, avec leur clé d’accès et leurs événements de sécurité.",
  "Security events: 12 months.": "Événements de sécurité : 12 mois.",
  "Finished matches stay in the other players' history. Once your account is deleted, your seats show “Deleted player” and are no longer linked to you.":
    "Les parties terminées restent dans l'historique des autres joueurs. Une fois votre compte supprimé, vos places affichent « Joueur supprimé » et ne sont plus liées à vous.",
  "Backups: retention period not configured.": "Sauvegardes : durée de conservation non configurée.",
  "Backups: overwritten after {0} days.": "Sauvegardes : écrasées au bout de {0} jours.",
  "Cookies": "Cookies",
  "Beelot sets only two cookies: the session cookie, which keeps you signed in, and the XSRF-TOKEN cookie, which protects your requests against forgery. Both are strictly necessary, so Beelot asks for no consent. Your settings, such as the language and sound, stay in your browser and are never sent to Beelot.":
    "Beelot ne dépose que deux cookies : le cookie de session, qui vous garde connecté, et le cookie XSRF-TOKEN, qui protège vos requêtes contre la falsification. Tous deux sont strictement nécessaires : Beelot ne demande donc pas de consentement. Vos réglages, comme la langue et le son, restent dans votre navigateur et ne sont jamais envoyés à Beelot.",
  "Your rights": "Vos droits",
  "Access and portability: Settings → Download my data gives you a JSON file with your account, your security events and your matches.":
    "Accès et portabilité : Paramètres → Télécharger mes données vous donne un fichier JSON avec votre compte, vos événements de sécurité et vos parties.",
  "Erasure: Settings → Delete my account deletes your account, your passkey and your security events straight away. Your device may still offer the passkey: remove it in your device's settings.":
    "Effacement : Paramètres → Supprimer mon compte supprime immédiatement votre compte, votre clé d’accès et vos événements de sécurité. Votre appareil peut encore proposer la clé d’accès : supprimez-la dans les réglages de votre appareil.",
  "Rectification, restriction and objection: write to the privacy contact.":
    "Rectification, limitation et opposition : écrivez au contact données personnelles.",
  "If you think your data is mishandled, you can complain to the CNIL, the French data protection authority:":
    "Si vous estimez que vos données sont mal traitées, vous pouvez adresser une réclamation à la CNIL, l'autorité française de protection des données :",
  "Deleted player": "Joueur supprimé",

  // Settings
  "Preferences": "Préférences",
  "Language": "Langue",
  "Choose the language of the interface.": "Choisissez la langue de l'interface.",
  "Sound effects": "Effets sonores",
  "Enable optional game sounds.": "Activer les sons de jeu facultatifs.",
  "Effects": "Effets",
  "Choose how much the table moves.": "Choisissez l'intensité des animations de la table.",
  "Full": "Complets",
  "Reduced": "Réduits",
  "No effects": "Aucun effet",
  "Table theme": "Thème de la table",
  "Choose the colours of the table and the card back.": "Choisissez les couleurs de la table et du dos des cartes.",
  "Comic": "BD",
  "Classic green": "Vert classique",
  "Midnight": "Minuit",
  "Sunset": "Coucher de soleil",
  "Cards always use both a suit symbol and colour. Controls support touch and keyboard use.":
    "Les cartes utilisent toujours à la fois un symbole et une couleur. Les commandes fonctionnent au toucher et au clavier.",

  // Game board (client-built)
  "Contract: {0} {1}{2}": "Contrat : {0} {1}{2}",
  " · coinched": " · coinché",
  "{0} are trump": "{0} est l'atout",
  "Contract made": "Contrat rempli",
  "Contract failed": "Contrat chuté",
  "Contract made.": "Contrat rempli.",
  "Contract failed.": "Contrat chuté.",
  "{0}'s turn · {1} of 8 tricks completed": "Au tour de {0} · {1} plis sur 8 terminés",
  "{0} win the match!": "{0} remporte la partie !",
  "Match score — North–South {0} · East–West {1} · to {2}": "Score de la partie — Nord–Sud {0} · Est–Ouest {1} · en {2}",
  " Belote/Rebelote bonus: {0} points.": " Bonus Belote/Rebelote : {0} points.",
  "Dealing the remaining cards…": "Distribution des cartes restantes…",
  "Dealer": "Donneur",
  "{0} · You": "{0} · Vous",
  "{0} · {1} bots": "{0} · bots : {1}",
  "{0}, {1}, {2}{3}": "{0}, {1}, {2}{3}",
  ", active player": ", joueur actif",
  "+{0} pts": "+{0} pts",
  "{0} wins the trick for {1} points": "{0} remporte le pli pour {1} points",
  "Play {0}": "Jouer {0}",
  "{0} of {1}": "{0} de {1}",
  "Coinche doubles the awarded scores. {0}": "La coinche double les scores attribués. {0}",
  "A team that takes every trick makes a capot and scores 250 instead of 162; in Contrée, a declaring team that makes a capot also scores its bid.":
    "Une équipe qui remporte tous les plis fait capot et marque 250 points au lieu de 162 ; à la Contrée, l'équipe preneuse qui fait capot marque aussi son annonce.",
  "The first team to reach the match target (1,000 points unless the players choose 500, 1,500 or 2,000) wins the match. When both teams reach it in the same round, the higher score wins; an exact tie is settled by another round.":
    "La première équipe à atteindre l'objectif de la partie (1 000 points, sauf si les joueurs choisissent 500, 1 500 ou 2 000) remporte la partie. Si les deux équipes l'atteignent lors de la même donne, le score le plus élevé l'emporte ; en cas d'égalité parfaite, une donne supplémentaire départage les équipes.",
  "Capot! {0} take every trick and score 250.": "Capot ! {0} remportent tous les plis et marquent 250 points.",
  "Capot! {0} take every trick and score 250 plus their {1} bid.":
    "Capot ! {0} remportent tous les plis et marquent 250 points plus leur annonce de {1}.",
  "North–South: {0} card points + {1} Dix de der + {2} Belote = {3}. East–West: {4} card points + {5} Dix de der + {6} Belote = {7}.":
    "Nord–Sud : {0} points de cartes + {1} de dix de der + {2} de Belote = {3}. Est–Ouest : {4} points de cartes + {5} de dix de der + {6} de Belote = {7}.",

  // Server messages
  "Eight cards have been dealt. Bid from 80 to 160 or pass.": "Huit cartes ont été distribuées. Annoncez de 80 à 160 ou passez.",
  "Five cards have been dealt. Accept the upturned suit or pass.": "Cinq cartes ont été distribuées. Prenez à la couleur retournée ou passez.",
  "Everyone passed. The cards have been redealt.": "Tout le monde a passé. Les cartes ont été redistribuées.",
  "Everyone passed twice. The cards have been redealt.": "Tout le monde a passé deux fois. Les cartes ont été redistribuées.",
  "Bot game not found": "Partie contre les bots introuvable",
  "This match has ended. Start a rematch.": "Cette partie est terminée. Lancez une revanche.",
  "The auction is still in progress.": "Les enchères sont toujours en cours.",
  "A Belote table needs four players.": "Une table de belote nécessite quatre joueurs.",
  "Choose a contract value and suit in Contrée.": "Choisissez une valeur et une couleur de contrat en Contrée.",
  "In the first round, you may only accept the upturned suit.": "Au premier tour, vous ne pouvez que prendre à la couleur retournée.",
  "Choose a suit other than the upturned suit.": "Choisissez une couleur autre que la couleur retournée.",
  "Choose a trump suit for the contract.": "Choisissez une couleur d'atout pour le contrat.",
  "A Contrée bid must be from 80 to 160 in steps of 10.": "Une annonce de Contrée doit aller de 80 à 160 par pas de 10.",
  "Your bid must be higher than the current contract.": "Votre annonce doit être supérieure au contrat en cours.",
  "Only an opponent of the declaring team may coinche.": "Seul un adversaire de l'équipe déclarante peut coincher.",
  "Contract bids are only available in Contrée.": "Les annonces de contrat n'existent qu'en Contrée.",
  "The auction has ended.": "Les enchères sont terminées.",
  "It is not your turn to bid.": "Ce n'est pas votre tour d'annoncer.",
  "The invitation code is invalid.": "Le code d'invitation est invalide.",
  "The auction has not finished.": "Les enchères ne sont pas terminées.",
  "You are not authorized for this table.": "Vous n'êtes pas autorisé à accéder à cette table.",
  "This game has not started.": "Cette partie n'a pas commencé.",
  "This table does not exist.": "Cette table n'existe pas.",
  "Enter a player name.": "Saisissez un nom de joueur.",
  "This table has already started.": "Cette table a déjà commencé.",
  "This table is full.": "Cette table est complète.",
  "That seat is taken.": "Cette place est déjà prise.",
  "That seat does not exist.": "Cette place n'existe pas.",
  "Seats are assigned at public tables.": "Les places sont attribuées aux tables publiques.",
  "A bot has taken over this seat for the rest of the match.": "Un bot a repris cette place pour le reste de la partie.",
  "Only the table owner can start the game.": "Seul le propriétaire de la table peut lancer la partie.",
  "Four ready players are required to start the game.": "Quatre joueurs prêts sont nécessaires pour lancer la partie.",
  "Every seated player must be ready to start with bots.": "Tous les joueurs assis doivent être prêts pour démarrer avec des bots.",
  "Only the table owner can change the turn timer.": "Seul le propriétaire de la table peut modifier le minuteur de tour.",
  "The turn timer cannot change after the game starts.": "Le minuteur de tour ne peut plus être modifié une fois la partie commencée.",
  "Choose no timer, 30 seconds, or 60 seconds.": "Choisissez sans minuteur, 30 secondes ou 60 secondes.",
  "You are not seated at this table.": "Vous n'êtes pas assis à cette table.",
  "It is not your turn to play.": "Ce n'est pas votre tour de jouer.",
  "That card is not a legal play.": "Cette carte n'est pas un coup légal.",
  "This round has ended.": "Cette manche est terminée.",
  "There is no completed trick to continue from.": "Il n'y a aucun pli terminé à ramasser.",
  "Unknown player": "Joueur inconnu"
};

// Server messages that embed names, suits or numbers. Handlers receive the regex captures.
const suitWord = "(CLUBS|DIAMONDS|HEARTS|SPADES|ALL_TRUMPS|NO_TRUMPS|Clubs|Diamonds|Hearts|Spades|All trumps|No trumps)";
const frenchPatterns = (suitWord) => [
  [/^(.+) is deciding\.$/, (name) => (name === "You" ? "Vous réfléchissez." : `${name} réfléchit.`)],
  [new RegExp(`^(.+) bids (\\d+) ${suitWord}\\.$`), (name, value, suit) =>
    `${name === "You" ? "Vous annoncez" : `${name} annonce`} ${value} ${suitName(suit, false)}.`],
  [/^(.+) coinches the contract\.$/, (name) => `${name === "You" ? "Vous coinchez" : `${name} coinche`} le contrat.`],
  [new RegExp(`^The (\\d+) ${suitWord} contract is accepted\\.$`), (value, suit) =>
    `Le contrat de ${value} ${suitName(suit, false)} est accepté.`],
  [new RegExp(`^Everyone passed\\. Choose any trump suit except ${suitWord}\\.$`), (suit) =>
    `Tout le monde a passé. Choisissez n'importe quelle couleur d'atout sauf ${suitName(suit, false)}.`],
  [/^(.+) declares Belote\.$/, (name) => `${name === "You" ? "Vous annoncez" : `${name} annonce`} Belote.`],
  [/^(.+) declares Rebelote: 20 bonus points\.$/, (name) =>
    `${name === "You" ? "Vous annoncez" : `${name} annonce`} Rebelote : 20 points de bonus.`],
  [/^Bot game not found: (.+)$/, () => "Partie contre les bots introuvable."],
  [new RegExp(`^(\\d+) ${suitWord}$`), (value, suit) => `${value} ${suitName(suit)}`],
  [new RegExp(`^${suitWord}$`), (suit) => suitName(suit)],
  [/^(\d+) cards?$/, (count) => `${count} ${Number(count) <= 1 ? "carte" : "cartes"}`]
];

const frenchLocale = {
  dictionary: fr,
  suits: frenchSuits,
  rankNames: frenchRankNames,
  rankLabels: frenchRankLabels,
  cardName: (rank, suit) => `${frenchRankNames[rank] ?? rank} de ${suit}`.replace(/^./, (letter) => letter.toUpperCase()),
  patterns: frenchPatterns
};

const locales = { fr: frenchLocale, ...(window.beelotLocales ?? {}) };
const locale = locales[language];
const serverPatterns = locale ? locale.patterns(suitWord) : [];

// Suit name (any language, lower case) -> symbol, used to decorate call bubbles.
const suitSymbols = {};
const symbolBySuit = { clubs: "♣", diamonds: "♦", hearts: "♥", spades: "♠" };
Object.entries(symbolBySuit).forEach(([suit, symbol]) => { suitSymbols[suit] = symbol; });
Object.values(locales).forEach((entry) => Object.entries(entry.suits).forEach(([suit, word]) => {
  suitSymbols[word] = symbolBySuit[suit];
}));
const suitTail = new RegExp(`(${Object.keys(suitSymbols).join("|")})$`, "i");

function translateServer(text) {
  for (const [pattern, handler] of serverPatterns) {
    const match = pattern.exec(text);
    if (match) return handler(...match.slice(1));
  }
  return null;
}

function t(text, ...args) {
  let out = text;
  if (locale && typeof text === "string") {
    out = locale.dictionary[text] ?? translateServer(text) ?? text;
  }
  return args.length ? out.replace(/\{(\d+)\}/g, (_, index) => args[Number(index)]) : out;
}

// Suit names arrive as enum values (HEARTS) or display names (Hearts).
function suitName(suit, capitalise = true) {
  if (!suit) return "";
  const key = String(suit).toLowerCase().replace("_", " ");
  const name = locale ? locale.suits[key] ?? key : key;
  return capitalise ? name.charAt(0).toUpperCase() + name.slice(1) : name;
}

function rankLabel(rank) {
  return locale ? locale.rankLabels[rank] ?? rank : rank;
}

function cardName(card) {
  const suit = suitName(card.suit, false);
  return locale ? locale.cardName(card.rank, suit) : `${card.rank} of ${suit}`;
}

// A player's own name is shown as typed; the built-in "You" seat and a deleted player's seat are localised.
function playerName(name) {
  return name === "You" || name === "Deleted player" ? t(name) : name;
}

function collapseWhitespace(text) {
  return text.replace(/\s+/g, " ").trim();
}

// Translates the static markup once at load. Dynamic text goes through t() in app.js.
function translateStaticMarkup() {
  document.documentElement.lang = language;
  if (!locale) return;
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => (node.parentElement.closest("script, style") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT)
  });
  const nodes = [];
  while (walker.nextNode()) nodes.push(walker.currentNode);
  nodes.forEach((node) => {
    const source = collapseWhitespace(node.nodeValue);
    const translated = source && locale.dictionary[source];
    if (translated) node.nodeValue = node.nodeValue.replace(node.nodeValue.trim(), () => translated);
  });
  document.querySelectorAll("[aria-label], [placeholder], [title]").forEach((element) => {
    ["aria-label", "placeholder", "title"].forEach((attribute) => {
      if (element.hasAttribute(attribute)) element.setAttribute(attribute, t(element.getAttribute(attribute)));
    });
  });
  document.querySelector('meta[name="description"]')?.setAttribute("content", t("Start a game of Belote."));
  document.querySelectorAll("#owner-name, #join-name, #public-name").forEach((input) => { input.value = t(input.value); });
}

function setLanguage(next) {
  if (!supportedLanguages.includes(next) || next === language) return;
  try { window.localStorage.setItem(languageKey, next); } catch (_) { /* storage unavailable */ }
  window.location.reload();
}

translateStaticMarkup();
