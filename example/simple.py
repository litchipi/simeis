import json
import urllib.request

URL = "http://0.0.0.0:8080"

def get(path):
    reply = urllib.request.urlopen(URL + path)
    data = json.loads(reply.read().decode())
    return data

got = get("/ping")
print(got)

# TODO Créer un meilleur example simple comme celui-ci dans une doc "Getting started"
# 1 - Créer un joueur
# 2 - Acheter un vaisseau
# 3 - Recruter un pilote (et on l'assigne au module)
# 4 - Scanner les planètes aux alentours
# 5 - Acheter un module correspondant (Miner si solide, GasSucker si gazeux)
# 6 - Recruter un opérateur pour assigner au module
# 
# 7 - Aller sur la planète
# 8 - Démarrer l'extraction
# 9 - Attendre la fin de l'extraction
# 10- Revenir sur la station
# 11- Décharger le cargo sur la station
# 12- Recruter un Trader (s'il n'y en a pas déjà un)
# 13- Vendre toutes les resources extraites
# 14- Acheter du carburant
# 15- Recharger le carburant
# 16- Acheter des plaques
# 17- Réparer la coque
# 18- Reboucler à l'étape 7
