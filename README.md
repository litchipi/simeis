# Simeis

Jeu par API

## Observatoire (mode spectateur)

Le serveur embarque un tableau de bord temps réel sur
[`/spectate`](http://localhost:8080/spectate) : carte de la galaxie
(stations, planètes, vaisseaux en vol avec leur trajectoire), classement des
joueurs, prix du marché et fil d'événements. Aucune dépendance, aucune
configuration : lancez le serveur et ouvrez la page — idéal à projeter
pendant une partie.

- `/spectate?follow=<nom>` : centre la vue sur la flotte d'un joueur
- `/spectate/data` : le snapshot JSON utilisé par le tableau de bord,
  librement utilisable par vos propres outils
