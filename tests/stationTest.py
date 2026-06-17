
import argparse
import os
import random
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.insert(0, ROOT)

from sdk import SimeisSDK

DEFAULT_SERVER = '78.123.114.124'
DEFAULT_PORT = 42666


def main(username_prefix='station_test'):
    username = f"{username_prefix}_{int(time.time() * 1000)}_{random.randrange(1000, 9999)}"
    sdk = SimeisSDK(username, DEFAULT_SERVER, DEFAULT_PORT)
    status = sdk.get_player_status()
    station_id = status['stations'][0]

    trader = sdk.hire_crew(station_id, 'trader')
    assert 'id' in trader
    trader_id = trader['id']
    print('Hired trader:', trader_id)

    sdk.assign_trader_to_station(station_id, trader_id)
    assert sdk.station_has_trader(station_id), 'Station should have a trader assigned'
    print('Trader assigned to station')

    print('stationTest PASSED')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Run the station scenario test against Simeis server')
    parser.add_argument('--server', default=DEFAULT_SERVER, help='Simeis server IP')
    parser.add_argument('--port', type=int, default=DEFAULT_PORT, help='Simeis server port')
    args = parser.parse_args()
    DEFAULT_SERVER = args.server
    DEFAULT_PORT = args.port
    main()
