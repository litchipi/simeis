import os
import random
import string
import sys
import tempfile

from sdk import SimeisError, SimeisSDK

HOST = os.environ.get("SIMEIS_HOST", "127.0.0.1")
PORT = int(os.environ.get("SIMEIS_PORT", "9345"))


def rand_username(prefix):
    suffix = "".join(random.choices(string.ascii_lowercase + string.digits, k=8))
    return f"{prefix}{suffix}"


def new_player(prefix):
    return SimeisSDK(rand_username(prefix), HOST, PORT)


# En mode "testing", le serveur n'avance le jeu que sur demande explicite
# (endpoint /tick), il n'y a pas de boucle temps reel a attendre.
def advance_until_idle(sdk, ship_id, max_ticks=2000):
    for _ in range(max_ticks):
        status = sdk.get_ship_status(ship_id)
        if status["state"] == "Idle":
            return status
        sdk.post("/tick")
    raise SimeisError(f"le vaisseau {ship_id} n'est jamais redevenu Idle")


def scenario_buy_ship_and_module():
    # On cree un nouveau joueur, on achete un vaisseau puis un module de minage
    # L'argent doit diminuer a chaque transaction
    sdk = new_player("shipbuyer")
    status = sdk.get_player_status()
    money_start = status["money"]
    assert money_start > 0, "le joueur doit demarrer avec de l'argent"

    sta = status["stations"][0]
    ships = sdk.shop_list_ship(sta)
    assert len(ships) > 0, "le shipyard doit proposer au moins un vaisseau"

    sdk.buy_ship(sta, ships[0]["id"])
    status = sdk.get_player_status()
    assert (
        len(status["ships"]) == 1
    ), "le vaisseau achete doit apparaitre chez le joueur"
    money_after_ship = status["money"]
    assert (
        money_after_ship < money_start
    ), "l'argent doit avoir diminue apres l'achat du vaisseau"

    ship_id = status["ships"][0]["id"]
    sdk.buy_module_on_ship(sta, ship_id, "Miner")
    money_after_module = sdk.get_player_status()["money"]
    assert (
        money_after_module < money_after_ship
    ), "l'argent doit avoir encore diminue apres l'achat du module"


def scenario_navigation():
    # On achete un vaisseau, on lui assigne un pilote, puis on navigue vers une destination
    # Le vaisseau doit changer d'etat, de position, et consommer du carburant
    sdk = new_player("navigator")
    status = sdk.get_player_status()
    sta = status["stations"][0]
    origin = tuple(sdk.get_station_status(sta)["position"])

    sdk.buy_ship(sta, sdk.shop_list_ship(sta)[0]["id"])
    ship_id = sdk.get_player_status()["ships"][0]["id"]

    pilot = sdk.hire_crew(sta, "pilot")
    sdk.assign_crew_to_ship(sta, ship_id, pilot["id"], "pilot")

    fuel_before = sdk.get_ship_status(ship_id)["fuel_tank"]

    dest = (origin[0] + 5, origin[1] + 5, origin[2] + 5)
    x, y, z = dest
    sdk.post(f"/ship/{ship_id}/navigate/{x}/{y}/{z}")

    ship_status = advance_until_idle(sdk, ship_id)
    assert ship_status["state"] == "Idle", "le vaisseau doit etre revenu a l'etat Idle"
    assert (
        tuple(ship_status["position"]) == dest
    ), "le vaisseau doit etre arrive a destination"
    assert (
        ship_status["fuel_tank"] < fuel_before
    ), "le carburant doit avoir diminue pendant le trajet"


def scenario_market_trade():
    # On embauche un trader assigne a la station, on achete puis on revend une ressource
    # L'argent et le stock de la station doivent varier dans le bon sens, avec des frais preleves
    sdk = new_player("trader")
    sta = sdk.get_player_status()["stations"][0]

    trader = sdk.hire_crew(sta, "trader")
    sdk.assign_trader_to_station(sta, trader["id"])

    money_before_buy = sdk.get_player_status()["money"]
    bought = sdk.buy_resource(sta, "fuel", 50)
    assert bought["fees"] > 0, "des frais doivent etre preleves sur la transaction"
    money_after_buy = sdk.get_player_status()["money"]
    assert (
        money_after_buy < money_before_buy
    ), "l'argent doit diminuer apres un achat sur le marche"

    stock_after_buy = sdk.get_station_resources(sta).get("Fuel", 0)
    assert stock_after_buy > 0, "le carburant achete doit etre stocke dans la station"

    sdk.sell_resource(sta, "fuel", stock_after_buy / 2)
    money_after_sell = sdk.get_player_status()["money"]
    assert (
        money_after_sell > money_after_buy
    ), "l'argent doit augmenter apres une vente sur le marche"

    stock_after_sell = sdk.get_station_resources(sta).get("Fuel", 0)
    assert stock_after_sell < stock_after_buy, "le stock doit diminuer apres la vente"


SCENARIOS = [
    scenario_buy_ship_and_module,
    scenario_navigation,
    scenario_market_trade,
]


def main():
    # Chaque joueur de test ecrit un fichier <username>.json dans le repertoire courant
    # On isole ca dans un dossier temporaire pour ne pas polluer le repo
    os.chdir(tempfile.mkdtemp(prefix="simeis-functional-"))

    failures = []
    for scenario in SCENARIOS:
        name = scenario.__name__
        print(f"=== {name} ===")
        try:
            scenario()
        except (AssertionError, SimeisError) as exc:
            failures.append((name, exc))
            print(f"FAIL {name}: {exc}")
        else:
            print(f"PASS {name}")
        print()

    if failures:
        print(f"{len(failures)}/{len(SCENARIOS)} scenario(s) failed")
        sys.exit(1)

    print(f"All {len(SCENARIOS)} functional scenarios passed")


if __name__ == "__main__":
    main()
