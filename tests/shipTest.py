
import argparse
import os
import random
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.insert(0, ROOT)

from sdk import SimeisSDK, SimeisError

DEFAULT_SERVER = '78.123.114.124'
DEFAULT_PORT = 42666 


def main(username_prefix='ship_test'):
    username = f"{username_prefix}_{int(time.time() * 1000)}_{random.randrange(1000, 9999)}"
    sdk = SimeisSDK(username, DEFAULT_SERVER, DEFAULT_PORT)
    status = sdk.get_player_status()

    assert status['money'] == 72000.0, f"Expected initial money 72000.0, got {status['money']}"
    station_id = status['stations'][0]

    available_ships = sdk.shop_list_ship(station_id)
    assert len(available_ships) > 0, 'No ships available in shipyard'
    ship = available_ships[0]
    print('Buying ship', ship)

    purchase = sdk.buy_ship(station_id, ship['id'])
    assert 'id' in purchase
    new_ship_id = purchase['id']

    status_after_ship = sdk.get_player_status()
    assert status_after_ship['money'] < status['money']
    print('Money after ship purchase:', status_after_ship['money'])

    module_choice = 'Miner'
    try:
        module_purchase = sdk.buy_module_on_ship(station_id, new_ship_id, module_choice)
    except SimeisError as exc:
        module_choice = 'GasSucker'
        module_purchase = sdk.buy_module_on_ship(station_id, new_ship_id, module_choice)

    assert 'id' in module_purchase and 'cost' in module_purchase
    print(f'Bought module {module_choice}:', module_purchase)

    status_after_module = sdk.get_player_status()
    assert status_after_module['money'] < status_after_ship['money']
    print('Money after module purchase:', status_after_module['money'])

    print('shipTest PASSED')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Run the ship scenario test against Simeis server')
    parser.add_argument('--server', default=DEFAULT_SERVER, help='Simeis server IP')
    parser.add_argument('--port', type=int, default=DEFAULT_PORT, help='Simeis server port')
    args = parser.parse_args()
    DEFAULT_SERVER = args.server
    DEFAULT_PORT = args.port
    main()
