#!/usr/bin/env python3

import json
import subprocess
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path

BASE_URL = "http://localhost:9345"

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
SERVER_BIN = PROJECT_ROOT / "target" / "debug" / "simeis-server"


def api_call(method: str, path: str, key: str | None = None) -> dict:
    url = BASE_URL + path
    req = urllib.request.Request(url, method=method)

    if key is not None:
        req.add_header("Simeis-Key", key)

    try:
        with urllib.request.urlopen(req) as resp:
            body = resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8")

    data = json.loads(body)
    return data


def check_ok(data: dict, context: str = "") -> dict:
    if data.get("error") != "ok":
        msg = data.get("error", "erreur inconnue")
        etype = data.get("type", "")
        raise AssertionError(
            f"[{context}] Erreur API inattendue : {msg} (type={etype}) | réponse complète : {data}"
        )
    return data


def print_step(msg: str) -> None:
    print(f"  --> {msg}")


def print_ok(msg: str) -> None:
    print(f"  [OK] {msg}")


def start_server() -> subprocess.Popen:
    print(f"Démarrage du serveur : {SERVER_BIN}")

    if not SERVER_BIN.exists():
        raise FileNotFoundError(
            f"Binaire introuvable : {SERVER_BIN}\n"
            "Compilez le serveur avec : cargo build --features testing"
        )

    proc = subprocess.Popen(
        [str(SERVER_BIN)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        cwd=str(PROJECT_ROOT),
    )

    deadline = time.time() + 15.0
    while time.time() < deadline:
        try:
            data = api_call("GET", "/ping")
            if data.get("ping") == "pong":
                print("Serveur prêt.\n")
                return proc
        except Exception:
            pass
        time.sleep(0.2)

    proc.kill()
    raise RuntimeError("Le serveur n'a pas démarré dans les 15 secondes.")


def stop_server(proc: subprocess.Popen) -> None:
    print("\nArrêt du serveur...")
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    print("Serveur arrêté.")


def scenario_creation_et_achat_vaisseau() -> None:
    print("=== Scénario 1 : Création d'un joueur et achat d'un vaisseau ===")

    print_step("Création du joueur 'test-achat-vaisseau'")
    resp = api_call("POST", "/player/new/test-achat-vaisseau")
    check_ok(resp, "création joueur")
    player_id = resp["playerId"]
    key = resp["key"]
    print_ok(f"Joueur créé : ID={player_id}")

    print_step("Vérification de l'argent initial (72 000 crédits attendus)")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur")
    money_initiale = resp["money"]
    assert (
        money_initiale == 72000.0
    ), f"L'argent initial devrait être 72000, mais vaut {money_initiale}"
    print_ok(f"Argent initial correct : {money_initiale} crédits")

    print_step("Récupération de l'identifiant de la station de départ")
    stations = resp["stations"]
    assert len(stations) > 0, "Le joueur devrait avoir au moins une station"
    station_id = stations[0]
    print_ok(f"Station de départ trouvée : ID={station_id}")

    print_step("Listage des vaisseaux disponibles dans le chantier naval")
    resp = api_call("GET", f"/station/{station_id}/shipyard/list", key=key)
    check_ok(resp, "liste chantier naval")
    ships_dispo = resp["ships"]
    assert len(ships_dispo) > 0, "Il devrait y avoir au moins un vaisseau à vendre"
    premier_vaisseau = ships_dispo[0]
    vaisseau_id = premier_vaisseau["id"]
    prix_vaisseau = premier_vaisseau["price"]
    print_ok(
        f"Vaisseaux disponibles : {len(ships_dispo)}, premier ID={vaisseau_id}, prix={prix_vaisseau}"
    )

    print_step(f"Achat du vaisseau ID={vaisseau_id} pour {prix_vaisseau} crédits")
    resp = api_call(
        "POST", f"/station/{station_id}/shipyard/buy/{vaisseau_id}", key=key
    )
    check_ok(resp, "achat vaisseau")
    nouveau_vaisseau_id = resp["id"]
    print_ok(f"Vaisseau acheté avec succès, nouvel ID={nouveau_vaisseau_id}")

    print_step("Vérification de la diminution d'argent après l'achat")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur après achat")
    money_apres = resp["money"]
    assert (
        money_apres < money_initiale
    ), f"L'argent devrait avoir diminué après l'achat : avant={money_initiale}, après={money_apres}"
    depense = money_initiale - money_apres
    assert (
        abs(depense - prix_vaisseau) < 1.0
    ), f"La dépense ({depense}) devrait correspondre au prix du vaisseau ({prix_vaisseau})"
    print_ok(f"Argent après achat : {money_apres} crédits (dépense : {depense})")

    print_step("Vérification que le vaisseau est bien associé au joueur")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur vaisseaux")
    vaisseaux_joueur = resp["ships"]
    ids_vaisseaux = [v["id"] for v in vaisseaux_joueur]
    assert (
        nouveau_vaisseau_id in ids_vaisseaux
    ), f"Le vaisseau ID={nouveau_vaisseau_id} devrait apparaître dans {ids_vaisseaux}"
    print_ok(f"Vaisseau ID={nouveau_vaisseau_id} bien présent dans la flotte du joueur")

    print(">>> Scénario 1 : SUCCÈS\n")


def scenario_equipage_et_modules() -> None:
    print("=== Scénario 2 : Gestion de l'équipage et équipement du vaisseau ===")

    print_step("Création du joueur 'test-equipage'")
    resp = api_call("POST", "/player/new/test-equipage")
    check_ok(resp, "création joueur")
    player_id = resp["playerId"]
    key = resp["key"]
    print_ok(f"Joueur créé : ID={player_id}")

    print_step("Récupération de la station et achat d'un vaisseau")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur")
    station_id = resp["stations"][0]

    resp = api_call("GET", f"/station/{station_id}/shipyard/list", key=key)
    check_ok(resp, "liste chantier naval")
    vaisseau_id_catalogue = resp["ships"][0]["id"]

    resp = api_call(
        "POST", f"/station/{station_id}/shipyard/buy/{vaisseau_id_catalogue}", key=key
    )
    check_ok(resp, "achat vaisseau")
    vaisseau_id = resp["id"]
    print_ok(f"Vaisseau acheté : ID={vaisseau_id}")

    print_step("Embauche d'un Pilote pour la station")
    resp = api_call("POST", f"/station/{station_id}/crew/hire/Pilot", key=key)
    check_ok(resp, "embauche pilote")
    pilote_id = resp["id"]
    print_ok(f"Pilote embauché : ID={pilote_id}")

    print_step(f"Assignation du pilote ID={pilote_id} au vaisseau ID={vaisseau_id}")
    resp = api_call(
        "POST",
        f"/station/{station_id}/crew/assign/{pilote_id}/ship/{vaisseau_id}/pilot",
        key=key,
    )
    check_ok(resp, "assignation pilote")
    print_ok("Pilote assigné au vaisseau")

    print_step("Embauche d'un Opérateur pour la station")
    resp = api_call("POST", f"/station/{station_id}/crew/hire/Operator", key=key)
    check_ok(resp, "embauche opérateur")
    operateur_id = resp["id"]
    print_ok(f"Opérateur embauché : ID={operateur_id}")

    print_step(f"Achat d'un module Miner pour le vaisseau ID={vaisseau_id}")
    resp = api_call(
        "POST",
        f"/station/{station_id}/shop/modules/{vaisseau_id}/buy/Miner",
        key=key,
    )
    check_ok(resp, "achat module Miner")
    module_id = resp["id"]
    cout_module = resp["cost"]
    print_ok(f"Module Miner acheté : ID={module_id}, coût={cout_module}")

    print_step(
        f"Assignation de l'opérateur ID={operateur_id} au module ID={module_id} "
        f"du vaisseau ID={vaisseau_id}"
    )
    resp = api_call(
        "POST",
        f"/station/{station_id}/crew/assign/{operateur_id}/ship/{vaisseau_id}/{module_id}",
        key=key,
    )
    check_ok(resp, "assignation opérateur au module")
    print_ok("Opérateur assigné au module du vaisseau")

    print_step("Vérification de l'état final du vaisseau")
    resp = api_call("GET", f"/ship/{vaisseau_id}", key=key)
    check_ok(resp, "statut vaisseau final")

    assert (
        resp.get("pilot") is not None
    ), f"Le vaisseau devrait avoir un pilote assigné, mais 'pilot' vaut {resp.get('pilot')}"
    print_ok(f"Pilote bien présent sur le vaisseau : {resp['pilot']}")

    modules = resp.get("modules", {})
    assert (
        len(modules) > 0
    ), f"Le vaisseau devrait avoir au moins un module, mais 'modules' est vide : {modules}"

    module_key = str(module_id)
    assert (
        module_key in modules
    ), f"Le module ID={module_id} devrait être dans les modules du vaisseau : {list(modules.keys())}"
    module_data = modules[module_key]
    assert (
        module_data.get("operator") is not None
    ), f"Le module devrait avoir un opérateur assigné, mais 'operator' vaut {module_data.get('operator')}"
    print_ok(
        f"Module {module_id} bien présent avec opérateur : {module_data.get('operator')}"
    )

    print(">>> Scénario 2 : SUCCÈS\n")


def scenario_commerce_marche() -> None:
    print("=== Scénario 3 : Commerce sur le marché ===")

    print_step("Création du joueur 'test-rich-commerce'")
    resp = api_call("POST", "/player/new/test-rich-commerce")
    check_ok(resp, "création joueur riche")
    player_id = resp["playerId"]
    key = resp["key"]
    print_ok(f"Joueur créé : ID={player_id}")

    print_step("Récupération de la station de départ")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur")
    station_id = resp["stations"][0]
    money_initiale = resp["money"]
    print_ok(f"Station ID={station_id}, argent initial={money_initiale}")

    print_step("Embauche d'un Trader pour la station")
    resp = api_call("POST", f"/station/{station_id}/crew/hire/Trader", key=key)
    check_ok(resp, "embauche trader")
    trader_id = resp["id"]
    print_ok(f"Trader embauché : ID={trader_id}")

    print_step(f"Assignation du Trader ID={trader_id} à la station")
    resp = api_call(
        "POST",
        f"/station/{station_id}/crew/assign/{trader_id}/trading",
        key=key,
    )
    check_ok(resp, "assignation trader")
    print_ok("Trader assigné à la station")

    print_step("Consultation des prix du marché (endpoint public)")
    resp = api_call("GET", "/market/prices")
    check_ok(resp, "prix marché")
    assert (
        "Carbon" in resp
    ), f"Le marché devrait lister la ressource 'Carbon', réponse : {resp}"
    prix_carbon = resp["Carbon"]
    print_ok(f"Prix du marché récupérés, Carbon={prix_carbon} crédits/unité")

    quantite_achat = 10.0
    print_step(f"Achat de {quantite_achat} unités de Carbon sur le marché")
    resp = api_call(
        "POST",
        f"/market/{station_id}/buy/Carbon/{quantite_achat}",
        key=key,
    )
    check_ok(resp, "achat Carbon")
    argent_retire = resp.get("removed_money")
    assert (
        argent_retire is not None and argent_retire > 0
    ), f"L'achat devrait retirer de l'argent, removed_money={argent_retire}"
    print_ok(f"Achat réussi : {argent_retire} crédits débités (frais inclus)")

    print_step("Vérification de la diminution d'argent après l'achat")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur après achat")
    money_apres_achat = resp["money"]
    assert (
        money_apres_achat < money_initiale
    ), f"L'argent devrait avoir diminué : avant={money_initiale}, après={money_apres_achat}"
    print_ok(
        f"Argent après achat : {money_apres_achat} (réduit de {money_initiale - money_apres_achat:.2f})"
    )

    print_step("Consultation du taux de frais de la station")
    resp = api_call("GET", f"/market/{station_id}/fee_rate", key=key)
    check_ok(resp, "taux de frais")
    fee_rate = resp.get("fee_rate")
    assert (
        fee_rate is not None and fee_rate > 0.0
    ), f"Le taux de frais devrait être positif, fee_rate={fee_rate}"
    print_ok(f"Taux de frais récupéré : {fee_rate:.2%}")

    print_step(f"Revente de {quantite_achat} unités de Carbon sur le marché")
    resp = api_call(
        "POST",
        f"/market/{station_id}/sell/Carbon/{quantite_achat}",
        key=key,
    )
    check_ok(resp, "vente Carbon")
    argent_ajoute = resp.get("added_money")
    assert (
        argent_ajoute is not None and argent_ajoute > 0
    ), f"La vente devrait ajouter de l'argent, added_money={argent_ajoute}"
    print_ok(f"Vente réussie : {argent_ajoute} crédits encaissés (frais déduits)")

    print_step("Vérification de l'argent après la vente")
    resp = api_call("GET", f"/player/{player_id}", key=key)
    check_ok(resp, "statut joueur après vente")
    money_apres_vente = resp["money"]
    assert money_apres_vente > money_apres_achat, (
        f"L'argent devrait avoir augmenté après la vente : "
        f"avant vente={money_apres_achat}, après vente={money_apres_vente}"
    )
    print_ok(
        f"Argent après vente : {money_apres_vente} (gain : {money_apres_vente - money_apres_achat:.2f})"
    )

    print(">>> Scénario 3 : SUCCÈS\n")


def main() -> int:
    server_proc = None
    try:
        server_proc = start_server()
    except (FileNotFoundError, RuntimeError) as e:
        print(f"ERREUR : Impossible de démarrer le serveur : {e}", file=sys.stderr)
        return 1

    erreur = None
    try:
        scenario_creation_et_achat_vaisseau()
        scenario_equipage_et_modules()
        scenario_commerce_marche()
        print("=== Tous les tests fonctionnels ont réussi ! ===")
    except AssertionError as e:
        print(f"\nECHEC : Assertion non vérifiée : {e}", file=sys.stderr)
        erreur = e
    except Exception as e:
        print(
            f"\nECHEC : Erreur inattendue : {type(e).__name__} : {e}", file=sys.stderr
        )
        erreur = e
    finally:
        if server_proc is not None:
            stop_server(server_proc)

    return 0 if erreur is None else 1


if __name__ == "__main__":
    sys.exit(main())
