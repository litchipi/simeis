
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


def main(username_prefix='market_test'):
    username = f"{username_prefix}_{int(time.time() * 1000)}_{random.randrange(1000, 9999)}"
    sdk = SimeisSDK(username, DEFAULT_SERVER, DEFAULT_PORT)
    status = sdk.get_player_status()

    assert status['money'] == 72000.0, f"Expected initial money 72000.0, got {status['money']}"
    print('Initial money:', status['money'])

    station_id = status['stations'][0]
    before_money = status['money']

    if not sdk.station_has_trader(station_id):
        trader = sdk.hire_crew(station_id, 'trader')
        sdk.assign_trader_to_station(station_id, trader['id'])
        assert sdk.station_has_trader(station_id), 'Station should have a trader after assignment'

    buy = sdk.buy_resource(station_id, 'Fuel', 1.0)
    assert buy['removed_money'] > 0, 'Market buy should remove money'
    print('Bought Fuel:', buy)

    status_after_buy = sdk.get_player_status()
    assert status_after_buy['money'] < before_money
    print('Money after buy:', status_after_buy['money'])

    sell = sdk.sell_resource(station_id, 'Fuel', 1.0)
    assert sell['added_money'] > 0, 'Market sell should add money'
    print('Sold Fuel:', sell)

    status_after_sell = sdk.get_player_status()
    assert status_after_sell['money'] >= 0
    assert status_after_sell['money'] > status_after_buy['money']
    print('Money after sell:', status_after_sell['money'])

    print('marketTest PASSED')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Run the market scenario test against Simeis server')
    parser.add_argument('--server', default=DEFAULT_SERVER, help='Simeis server IP')
    parser.add_argument('--port', type=int, default=DEFAULT_PORT, help='Simeis server port')
    args = parser.parse_args()
    DEFAULT_SERVER = args.server
    DEFAULT_PORT = args.port
    main()
