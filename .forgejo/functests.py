import os
import sys
import math
import time
import json
import requests
import traceback
import urllib.parse

TESTS = []

FAILFAST=os.getenv("CI") is None

ITER_PERIOD = 20 / 1000

def functest(f):
    assert f.__name__.startswith("test_")
    name = f.__name__.removeprefix("test_").replace("_", " ").capitalize()
    global TESTS
    TESTS.append(f.__name__)
    def decorated(tester, *args, **kwargs):
        tester.current_test = name
        tester.trace = []
        f(tester, *args, **kwargs)
        tester.disp_ok()
    return decorated

class Tester:
    def __init__(self, host, port):
        self.host = host
        self.port = port
        self.key = None
        self.current_test = "No Test"
        self.trace = []
        self.saved_errors = {}
        self.indent = 30

    def addtrace(self, *args):
        self.trace.append(" ".join([ str(v) for v in args ]))

    def disp_error(self, exc):
        print("!!! Test", self.current_test, " "*(self.indent - len(self.current_test)), "ERR")

        errdata = "Trace of the test:\n"
        errdata += ("=" * 10) + " TRACE " + ("=" * 10) + "\n"
        for line in self.trace:
            errdata += line + "\n"
        errdata += ("=" * 10) + "  END  " + ("=" * 10) + "\n"
        tb_str = traceback.format_exception(
            exc,
            value=exc,
            tb=exc.__traceback__,
        )
        for line in tb_str:
            if (" assert " in line) or ("in test_" in line):
                errdata += line.strip() + "\n"
        errdata += "\n!!! Test {} failed\n".format(self.current_test)
        self.saved_errors[self.current_test] = errdata

    def disp_ok(self):
        print("  * Test", self.current_test, " "*(self.indent - len(self.current_test)), "OK")

    def request(self, endpoint, expcode=200, timeout=1, method="GET", headers={}, key=None, body=None, **kwargs):
        self.addtrace(method, endpoint, "expected code", expcode)
        if key is None:
            if self.key is not None:
                headers["Simeis-Key"] = self.key
        else:
            headers["Simeis-Key"] = str(key)

        url = f"http://{self.host}:{self.port}/{endpoint}"
        qry = urllib.parse.urlencode(kwargs)
        if len(qry) > 0:
            url += "?" + qry

        try:
            if body is None:
                headers["Content-Type"] = "application/x-www-form-urlencoded"
            else:
                headers["Content-Type"] = "application/json"
            if method == "GET":
                got = requests.get(url, headers=headers, timeout=timeout)
            elif method == "POST":
                self.addtrace("Body: \n", json.dumps(body, indent=2))
                got = requests.post(url, headers=headers, timeout=timeout, json=body)
        except requests.exceptions.Timeout:
            print(f"{method} {endpoint}:")
            print("\tRequest timed out, deadlock suspected in the server")
            print("")
            sys.exit(1)
        except requests.exceptions.ConnectionError:
            print("")
            print("Server panicked")
            print("===== TRACE =====")
            print("\n".join(self.trace))
            print("=================")
            print("Server panicked")
            print("")
            sys.exit(1)

        self.addtrace("HTTP code", got.status_code, "expected", expcode)
        assert got.status_code == expcode

        if expcode != 200:
            return got
        else:
            data = json.loads(got.text)
            self.addtrace("Decoded JSON data to", data)
            assert "error" in data.keys()
            return data

    def tick(self):
        self.request("/tick", method="POST")

    def tick_dur(self, ts):
        ntick = ts / ITER_PERIOD
        assert ntick > 0
        if (ntick % 1) > 0:
            ntick = int(ntick) + 1
        else:
            ntick = int(ntick)
        for _ in range(0, ntick):
            self.tick()

    def create_test_player(self, name=None):
        if name is None:
            name = "TestPlayer_" + self.current_test.replace(" ", "_").lower()
        got = self.post_ok("/player/new/" + name)
        self.key = self.assert_got(got, "key", None)
        self.id = self.assert_got(got, "playerId", None)
        self.tick()
        player = self.get_ok(f"/player/{self.id}")
        self.station = list(self.assert_got(player, "stations", None))[0]
        self.tick()
        return player

    def buy_a_ship(self, retind=0):
        player = self.get_ok(f"/player/{self.id}")
        got = self.get_ok(f"/station/{self.station}/shipyard/list")
        shiplist = self.assert_got(got, "ships", None)
        assert len(shiplist) > 0
        for ship in shiplist:
            if ship["price"] <= player["money"]:
                self.post_ok(f"/station/{self.station}/shipyard/buy/" + str(ship["id"]))
                self.tick()
        after = self.get_ok(f"/player/{self.id}")
        assert len(after["ships"]) > 0
        ship = after["ships"][retind]
        return self.assert_got(ship, "id", None)

    def setup_crew(self, shipid, trader=True):
        pilot = self.post_ok(f"/station/{self.station}/crew/hire/pilot")
        pilotid = self.assert_got(pilot, "id", None)
        self.post_ok(f"/station/{self.station}/crew/assign/{pilotid}/ship/{shipid}/pilot")
        self.tick()

        if trader:
            trader = self.post_ok(f"/station/{self.station}/crew/hire/trader")
            traderid = self.assert_got(trader, "id", None)
            self.post_ok(f"/station/{self.station}/crew/assign/{traderid}/trading")
        self.tick()

    def assert_ok(self, endpoint, **kwargs):
        got = self.request(endpoint, **kwargs)
        self.addtrace("Expect this data to be OK")
        assert got["error"] == "ok"
        del got["error"]
        return got

    def assert_error(self, endpoint, errtype=None, method="POST", **kwargs):
        got = self.request(endpoint, method=method, **kwargs)
        self.addtrace("Expect this data to be ERR", errtype)
        assert "type" in got
        if errtype is None:
            self.addtrace("Expect this data to have any error")
            assert got["error"] != "ok"
        else:
            self.addtrace("Expect this data to have an error", errtype, "got", got["type"])
            assert got["type"] == errtype
        return got

    def get_ok(self, *args, **kwargs):
        return self.assert_ok(*args, method="GET", **kwargs)

    def get_error(self, *args, **kwargs):
        return self.assert_error(*args, method="GET", **kwargs)

    def post_ok(self, *args, **kwargs):
        return self.assert_ok(*args, method="POST", **kwargs)

    def post_error(self, *args, **kwargs):
        return self.assert_error(*args, method="POST", **kwargs)

    def assert_got(self, data, key, val, negate=False):
        if not negate:
            self.addtrace("Expect this data to have the key", key)
            assert key in data.keys()
        else:
            self.addtrace("Expect this data to NOT have the key", key)
            assert key not in data.keys()
            return None

        if val is not None:
            self.addtrace(f"Expect data[{key}] to have value {val}, got", data[key])
            assert data[key] == val
        return data[key]

    def assert_cmpf(self, a, b, eps=0.0000001):
        diff = abs(a - b)
        self.addtrace(f"Assert {diff} < {eps}")
        if a > b:
            self.addtrace("Left is higher")
        elif a < b:
            self.addtrace("Right is higher")
        assert diff < eps

    @functest
    def test_ping(self):
        got = self.get_ok("/ping")
        self.assert_got(got, "ping", "pong")

    @functest
    def test_create_player(self):
        self.get_error("/player/53", errtype="NoPlayerKey")
        self.get_error("/player/53", errtype="NoPlayerWithKey", key=12341234)

        got = self.post_ok("/player/new/Testuser")
        self.key = self.assert_got(got, "key", None)
        self.id = self.assert_got(got, "playerId", 11070862243173938738)

        pl2 = self.post_ok("/player/new/Testuser2")
        pl2id = self.assert_got(pl2, "playerId", None)
        self.post_error("/player/new/Testuser2")
        pl2_key = self.assert_got(pl2, "key", None)

        pl1 = self.get_ok(f"/player/{self.id}")
        self.assert_got(pl1, "money", 72000)

        got = self.get_ok(f"/player/{self.id}", key=pl2["key"])
        self.assert_got(got, "money", None, negate=True)

        pl2 = self.get_ok(f"/player/{pl2id}", key=pl2["key"])
        self.assert_got(pl2, "money", self.assert_got(pl1, "money", None))

    @functest
    def test_syslog(self):
        self.create_test_player()

        got = self.get_ok("/syslogs")
        nb = self.assert_got(got, "nb", 1)
        ev = self.assert_got(got, "events", None)[0]
        self.assert_got(ev, "type", "GameStarted")
        self.assert_got(ev, "event", "GameStarted")

    @functest
    def test_shipyard(self):
        self.create_test_player()

        got = self.get_ok(f"/player/{self.id}")
        beforemoney = self.assert_got(got, "money", None)

        got = self.get_ok(f"/station/{self.station}/shipyard/list")
        shiplist = self.assert_got(got, "ships", None)
        assert len(shiplist) == 3
        self.addtrace("Ship list", shiplist)
        ships = {}
        for ship in shiplist:
            self.assert_got(ship, "id", None)
            self.assert_got(ship, "modules", {})
            self.assert_got(ship, "reactor_power", None)
            self.assert_got(ship, "cargo_capacity", None)
            self.assert_got(ship, "fuel_tank_capacity", None)
            self.assert_got(ship, "hull_resistance", None)
            self.assert_got(ship, "price", None)
            ships[ship["id"]] = ship

        n = 0
        while n in ships.keys():
            n += 1
        self.post_error(f"/station/{self.station}/shipyard/buy/{n}", errtype=f"ShipNotFound({n})")

        sid_cant = [id for id, ship in ships.items() if ship["price"] > beforemoney ][0]
        self.post_error(f"/station/{self.station}/shipyard/buy/{sid_cant}",
            errtype="NotEnoughMoney(72000.0, {})".format(ships[sid_cant]["price"])
        )

        sid_can = [id for id, ship in ships.items() if ship["price"] <= beforemoney ][0]
        self.post_ok(f"/station/{self.station}/shipyard/buy/{sid_can}")
        after = self.get_ok(f"/player/{self.id}")
        aftermoney = self.assert_got(after, "money", None)
        ships = self.assert_got(after, "ships", None)
        assert len(ships) > 0
        assert ships[0]["id"] == sid_can
        assert aftermoney < beforemoney

        # TODO (#22)    Sell ship with given ID
        # TODO (#22)   Assert ship no longer in possession

    @functest
    def test_hire_crew(self):
        self.create_test_player()
        nidle = 0
        for ctype in ["pilot", "operator", "trader", "soldier"]:
            got = self.post_ok(f"/station/{self.station}/crew/hire/{ctype}")
            crew_id = self.assert_got(got, "id", None)

            stationstatus = self.get_ok(f"/station/{self.station}")
            idled = self.assert_got(stationstatus, "idle_crew", None)
            assert len(idled) == nidle + 1
            assert idled[str(crew_id)] == { "member_type": ctype.capitalize(), "rank": 1 }
            nidle += 1
        self.post_error(f"/station/{self.station}/crew/hire/noteist", errtype="InvalidArgument(\"crewtype\")")

    @functest
    def test_money(self):
        self.create_test_player()
        self.post_ok(f"/station/{self.station}/crew/hire/pilot")
        before = self.get_ok(f"/player/{self.id}")
        costs = self.assert_got(before, "costs", None)
        self.tick()
        self.tick()

        after = self.get_ok(f"/player/{self.id}")
        self.assert_cmpf(
            self.assert_got(after, "money", None),
            self.assert_got(before, "money", None) - (costs * ITER_PERIOD * 2),
            eps=costs * 0.075,
        )

    @functest
    def test_assign_crew(self):
        self.create_test_player()
        shipid = self.buy_a_ship()

        pilot = self.post_ok(f"/station/{self.station}/crew/hire/pilot")
        pilot2 = self.post_ok(f"/station/{self.station}/crew/hire/pilot")
        operator = self.post_ok(f"/station/{self.station}/crew/hire/operator")
        operator2 = self.post_ok(f"/station/{self.station}/crew/hire/operator")

        stationstatus = self.get_ok(f"/station/{self.station}")
        assert len(self.assert_got(stationstatus, "idle_crew", None)) == 4

        shipstatus = self.get_ok(f"/ship/{shipid}")
        stats = self.assert_got(shipstatus, "stats", None)
        speed = self.assert_got(stats, "speed", None)
        assert speed == 0

        operatorid = self.assert_got(operator, "id", None)
        self.post_error(f"/station/{self.station}/crew/assign/{operatorid}/ship/{shipid}/pilot", errtype="WrongCrewType(Pilot)")

        self.post_ok(f"/station/{self.station}/crew/assign/" + str(pilot["id"]) + f"/ship/{shipid}/pilot")
        stationstatus = self.get_ok(f"/station/{self.station}")
        assert len(self.assert_got(stationstatus, "idle_crew", None)) == 3

        shipstatus = self.get_ok(f"/ship/{shipid}")
        self.assert_got(shipstatus, "crew", { str(pilot["id"]): { "member_type": "Pilot", "rank": 1 }})
        self.assert_got(shipstatus, "pilot", pilot["id"])
        stats = self.assert_got(shipstatus, "stats", None)
        speed_after = self.assert_got(stats, "speed", None)
        assert speed_after > 0

        self.post_error(
            f"/station/{self.station}/crew/assign/" + str(pilot2["id"]) + f"/ship/{shipid}/pilot",
            errtype="CrewNotNeeded",
        )
        stationstatus = self.get_ok(f"/station/{self.station}")
        assert len(self.assert_got(stationstatus, "idle_crew", None)) == 3
        shipstatus_after = self.get_ok(f"/ship/{shipid}")
        assert shipstatus == shipstatus_after

        got = self.post_ok(f"/station/{self.station}/shop/modules/{shipid}/buy/miner")
        modid = self.assert_got(got, "id", 1)
        self.post_ok(f"/station/{self.station}/crew/assign/{operatorid}/ship/{shipid}/1")
        self.post_error(f"/station/{self.station}/crew/assign/{operatorid}/ship/{shipid}/1", errtype=f"CrewMemberNotIdle({operatorid})")
        self.post_error(f"/station/{self.station}/crew/assign/{operator2["id"]}/ship/{shipid}/1", errtype="CrewNotNeeded")

    @functest
    def test_travel(self):
        self.create_test_player()
        ship_id = self.buy_a_ship()
        self.setup_crew(ship_id)

        ship = self.get_ok(f"/ship/{ship_id}")
        shippos = ship["position"]

        self.post_error("/calc/navigation", errtype="NullDistance", body={
            "origin": shippos,
            "dest": shippos,
            "ship_stats": ship["stats"],
        })

        close = self.post_ok("/calc/navigation", body={
            "origin": shippos,
            "dest": (shippos[0]+1,shippos[1]+1, shippos[2]+1),
            "ship_stats": ship["stats"],
        })["costs"]
        dist = self.assert_got(close, "distance", None)
        assert dist is not None
        assert dist > 0.0

        far = self.post_ok("/calc/navigation", body={
            "origin": shippos,
            "dest": (shippos[0]+2,shippos[1]+2, shippos[2]+2),
            "ship_stats": ship["stats"],
        })["costs"]

        self.assert_got(far, "distance", 2 * self.assert_got(close, "distance", None))
        self.assert_got(far, "duration", 2 * self.assert_got(close, "duration", None))
        self.assert_got(far, "fuel_consumption", 2 * self.assert_got(close, "fuel_consumption", None))
        self.assert_got(far, "hull_usage", 2 * self.assert_got(close, "hull_usage", None))
        self.assert_got(close, "direction", self.assert_got(far, "direction", None))

        self.addtrace("Close distance costs:", close)
        assert int(close["duration"] / ITER_PERIOD) > 0
        niter=10
        nadd = int((niter * ITER_PERIOD) / close["duration"])
        self.addtrace("NADD", nadd)
        dst = [shippos[0] + nadd, shippos[1] + nadd, shippos[2] + nadd]

        # Simple travel
        self.addtrace("Testing a simple travel")
        before = self.get_ok(f"/ship/{ship_id}")
        src = self.assert_got(before, "position", None)
        self.assert_got(before, "state", "Idle")
        cost = self.post_ok(f"/ship/{ship_id}/navigate", body={
            "x": dst[0],
            "y": dst[1],
            "z": dst[2]
        })
        self.tick_dur(cost["duration"])
        self.addtrace("Arrived")
        after = self.get_ok(f"/ship/{ship_id}")
        self.assert_cmpf(
            before["fuel_tank"] - cost["fuel_consumption"],
            self.assert_got(after, "fuel_tank", None),
        )
        self.assert_cmpf(
            cost["hull_usage"],
            self.assert_got(after, "hull_decay", None),
        )
        self.assert_got(after, "position", dst)
        self.addtrace("Return")
        new_cost = self.post_ok(f"/ship/{ship_id}/navigate", body={
            "x": src[0], "y": src[1], "z": src[2]
        })
        for key in ["fuel_consumption", "hull_usage", "distance", "duration"]:
            exp = self.assert_got(cost, key, None)
            self.assert_got(new_cost, key, exp)
        self.tick_dur(cost["duration"])
        self.addtrace("Arrived")
        after = self.get_ok(f"/ship/{ship_id}")
        self.assert_cmpf(
            before["fuel_tank"] - (2.0*cost["fuel_consumption"]),
            self.assert_got(after, "fuel_tank", None),
        )
        self.assert_cmpf(
            2.0*cost["hull_usage"],
            self.assert_got(after, "hull_decay", None),
        )
        self.assert_got(after, "position", src)
        self.addtrace("Consumption as expected")

        # Travel, stop in the middle
        before = self.get_ok(f"/ship/{ship_id}")
        self.assert_got(before, "state", "Idle")
        beforepos = self.assert_got(before, "position", None)
        self.addtrace("Travel with in-middle stop")
        cost = self.post_ok(f"/ship/{ship_id}/navigate", body={
            "x": dst[0], "y": dst[1], "z": dst[2]
        })

        for _ in range(niter // 2):
            self.tick()

        during = self.get_ok(f"/ship/{ship_id}")
        assert self.assert_got(during, "state", None) != "Idle"

        pos = self.assert_got(during, "position", None)
        assert (pos[0] > shippos[0]) and (pos[1] > shippos[1]) and (pos[2] > shippos[2])
        assert (pos[0] < shippos[0]+nadd) and (pos[1] < shippos[1]+nadd) and (pos[2] < shippos[2]+nadd)

        stoppos = self.assert_got(
            self.post_ok(f"/ship/{ship_id}/navigation/stop"),
            "position",
            pos
        )
        stopped = self.get_ok(f"/ship/{ship_id}")
        pos = self.assert_got(stopped, "position", stoppos)
        assert (pos[0] > shippos[0]) and (pos[1] > shippos[1]) and (pos[2] > shippos[2])
        assert (pos[0] < shippos[0]+nadd) and (pos[1] < shippos[1]+nadd) and (pos[2] < shippos[2]+nadd)
        self.assert_got(stopped, "state", "Idle")

        costmid = self.post_ok("/calc/navigation", body={
            "ship_stats": ship["stats"],
            "origin": pos,
            "dest": dst,
        })["costs"]
        done = (cost["duration"] - self.assert_got(costmid, "duration", None)) / cost["duration"]
        for key in ["fuel_consumption", "hull_usage", "distance", "duration"]:
            midval = self.assert_got(costmid, key, None)
            assert cost[key] > midval
            consumption = cost[key] - midval
            exp = cost[key] * done
            self.addtrace(f"{key}: diff is {consumption} (expected {exp})")
            self.assert_cmpf(exp, consumption)

        new_cost = self.post_ok(f"/ship/{ship_id}/navigate", body={
            "x": dst[0], "y": dst[1], "z": dst[2]
        })
        for key in ["fuel_consumption", "hull_usage", "distance", "duration"]:
            self.assert_cmpf(
                costmid[key],
                self.assert_got(new_cost, key, None),
            )
        self.tick_dur(new_cost["duration"])

        after = self.get_ok(f"/ship/{ship_id}")
        self.assert_got(after, "state", "Idle")
        afterpos = self.assert_got(after, "position", None)

        assert self.assert_got(after, "fuel_tank", None) < self.assert_got(before, "fuel_tank", None)
        assert self.assert_got(after, "hull_decay", None) > self.assert_got(before, "hull_decay", None)
        self.addtrace("AFTER", after)
        self.addtrace("BEFORE", before)
        # FIXME    Fuel consumption when stopping in the middle is broken
        # self.assert_cmpf(
        #     self.assert_got(after, "fuel_tank", None),
        #     self.assert_got(before, "fuel_tank", None) - cost["fuel_consumption"],
        # )

        # FIXME    Hull usage when stopping in the middle is broken
        # self.assert_cmpf(
        #     self.assert_got(after, "hull_decay", None),
        #     self.assert_got(before, "hull_decay", None) + cost["hull_usage"],
        # )

        self.addtrace(
            "nadd = ", nadd,
            "diff coord = ",
            afterpos[0] - beforepos[0],
            afterpos[1] - beforepos[1],
            afterpos[2] - beforepos[2],
        )
        assert (afterpos[0] - beforepos[0]) == nadd
        assert (afterpos[1] - beforepos[1]) == nadd
        assert (afterpos[2] - beforepos[2]) == nadd

        cost = self.post_ok(f"/ship/{ship_id}/navigate", body={
            "x": shippos[0], "y": shippos[1], "z": shippos[2]
        })
        self.tick_dur(cost["duration"])

        back = self.get_ok(f"/ship/{ship_id}")
        self.addtrace("start", afterpos, "now", back["position"])
        self.addtrace("want", shippos, "got", back["position"])
        pos = self.assert_got(back, "position", shippos)

        self.assert_cmpf(
            self.assert_got(back, "fuel_tank", None),
            self.assert_got(after, "fuel_tank", None) - cost["fuel_consumption"],
        )
        self.assert_cmpf(
            self.assert_got(back, "hull_decay", None),
            self.assert_got(after, "hull_decay", None) + cost["hull_usage"],
        )

        got = self.get_ok("/syslogs")
        self.assert_got(got, "nb", 5)
        ev = self.assert_got(got, "events", None)[1]
        ty = self.assert_got(ev, "type", "ShipFlightFinished")
        self.assert_got(ev["event"], ty, ship_id)

    @functest
    def test_scan(self):
        self.create_test_player()

        scan = self.post_ok(f"/station/{self.station}/scan")

        planets = self.assert_got(scan, "planets", None)
        assert len(planets) > 0

        stations = self.assert_got(scan, "stations", None)
        assert len(stations) > 0
        assert any([sta["id"] == int(self.station) for sta in stations])

    @functest
    def test_extract(self):
        self.create_test_player("test-rich-extract")
        shipid = self.buy_a_ship()
        self.setup_crew(shipid, trader=False)

        got = self.post_ok(f"/station/{self.station}/shop/modules/{shipid}/buy/miner")
        modid = self.assert_got(got, "id", 1)
        operator = self.post_ok(f"/station/{self.station}/crew/hire/operator")
        opid = self.assert_got(operator, "id", None)
        self.post_ok(f"/station/{self.station}/crew/assign/{opid}/ship/{shipid}/{modid}")

        got = self.post_ok(f"/station/{self.station}/shop/modules/{shipid}/buy/gassucker")
        modid = self.assert_got(got, "id", 2)
        operator = self.post_ok(f"/station/{self.station}/crew/hire/operator")
        opid = self.assert_got(operator, "id", None)
        self.post_ok(f"/station/{self.station}/crew/assign/{opid}/ship/{shipid}/{modid}")
        self.addtrace("Got ship all set up")

        scan = self.post_ok(f"/station/{self.station}/scan")
        ship = self.get_ok(f"/ship/{shipid}")
        distances = []
        for planet in self.assert_got(scan, "planets", None):
            distances.append((planet, compute_distance(planet["position"], ship["position"])))

        best = sorted(distances, key=lambda f: f[1])[0][0]
        self.addtrace("Traveling to", best)

        dest = best["position"]
        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        self.tick_dur(cost["duration"])

        ship = self.get_ok(f"/ship/{shipid}")
        self.assert_got(ship, "state", "Idle")
        self.addtrace("Ship arrived")
        extraction_info = self.post_ok(f"/ship/{shipid}/extraction/start")
        extraction_rates = self.assert_got(extraction_info, "mining_rate", None)
        assert ("Carbon" in extraction_rates) or ("Hydrogen" in extraction_rates)
        self.tick()

        before = self.get_ok(f"/ship/{shipid}")
        cargob = self.assert_got(before, "cargo", None)
        self.tick()

        after = self.get_ok(f"/ship/{shipid}")
        cargoa = self.assert_got(after, "cargo", None)
        assert cargob["usage"] < cargoa["usage"]

        player = self.get_ok(f"/player/{self.id}")
        stationid = player["stations"][0]
        dest = self.assert_got(self.get_ok(f"/station/{stationid}"), "position", None)
        self.post_error(
            f"/ship/{shipid}/navigate", body={ "x": dest[0], "y": dest[1], "z": dest[2]},
            errtype="ShipNotIdle",
        )
        self.post_ok(f"/ship/{shipid}/extraction/stop")

    # Uses environment set up by previous test
    @functest
    def test_unload_cargo(self):
        player = self.get_ok(f"/player/{self.id}")
        stationid = player["stations"][0]
        ship = player["ships"][0]
        shipid = ship["id"]

        dest = self.get_ok(f"/station/{stationid}")["position"]
        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        self.tick_dur(cost["duration"])

        resname = list(ship["cargo"]["resources"].keys())[0]
        resamnt = ship["cargo"]["resources"][resname]
        resname = resname.lower()

        initprice = self.get_ok(f"/station/{stationid}/upgrades")
        initprice = self.assert_got(initprice, "cargo", 1.0)
        self.post_ok(f"/station/{stationid}/shop/cargo/buy/2000")
        afterprice = self.get_ok(f"/station/{stationid}/upgrades")
        afterprice = self.assert_got(afterprice, "cargo", None)
        assert afterprice > initprice

        cargobefore = self.get_ok(f"/station/{stationid}")["cargo"]
        usagebefore = self.assert_got(cargobefore, "usage", 0.0)
        to_unload = resamnt / 2
        got = self.post_ok(f"/ship/{shipid}/unload/{stationid}/{resname}/{to_unload}")
        self.assert_got(got, "unloaded", to_unload)
        cargoafter = self.get_ok(f"/station/{stationid}")["cargo"]
        usageafter = self.assert_got(cargoafter, "usage", None)
        assert usageafter > usagebefore

        shipafter = self.get_ok(f"/ship/{shipid}")
        shipcargo = self.assert_got(shipafter, "cargo", None)
        assert shipcargo["usage"] > 0

        got = self.post_ok(f"/ship/{shipid}/unload/{stationid}/all")
        self.assert_got(got, "emptied", True)
        cargoafter = self.get_ok(f"/station/{stationid}")["cargo"]
        usageafterafter = self.assert_got(cargoafter, "usage", None)
        assert usageafterafter > usageafter

        shipafter = self.get_ok(f"/ship/{shipid}")
        shipcargo = self.assert_got(shipafter, "cargo", None)
        assert shipcargo["usage"] == 0.0

    # Uses environment set up by previous test
    @functest
    def test_market_buy_sell(self):
        player = self.create_test_player("test-rich-market")
        resname = "Carbon"
        resamnt = 100

        self.post_error(f"/market/{self.station}/sell/{resname}/{resamnt}", errtype="NoTraderAssigned")

        trader = self.post_ok(f"/station/{self.station}/crew/hire/trader")
        traderid = self.assert_got(trader, "id", None)
        self.post_ok(f"/station/{self.station}/crew/assign/{traderid}/trading")

        self.post_error(
            f"/market/{self.station}/buy/{resname}/0",
            errtype="BuyNothing",
        )

        self.post_ok(f"/market/{self.station}/buy/{resname}/{resamnt}")

        player = self.get_ok(f"/player/{self.id}")
        costs = player["costs"]
        tstart = time.time()
        beforemoney = self.assert_got(player, "money", None)
        tx = self.post_ok(f"/market/{self.station}/sell/{resname}/{resamnt}")
        afterplayer = self.get_ok(f"/player/{self.id}")
        aftermoney = self.assert_got(afterplayer, "money", None)
        self.addtrace(
            "Should be the same:",
            aftermoney,
            beforemoney + tx["added_money"],
            "Difference: ",
            aftermoney - (beforemoney + tx["added_money"])
        )
        tdelta = time.time() - tstart
        assert aftermoney - (beforemoney + tx["added_money"]) < (tdelta * costs)

        cargo = self.get_ok(f"/station/{self.station}")["cargo"]
        self.assert_got(cargo, "resources", {resname.capitalize(): 0.0})
        self.assert_got(cargo, "usage", 0)

        self.post_error(
            f"/market/{self.station}/sell/{resname}/{resamnt}",
            errtype="SellNothing",
        )

        fee = self.get_ok(f"/market/{self.station}/fee_rate")
        feerate = self.assert_got(fee, "fee_rate", 0.25)
        carbon_tot = 30 / (1 - feerate)
        tx = self.post_ok(f"/market/{self.station}/buy/{resname}/{carbon_tot}")
        assert self.assert_got(tx, "removed_money", None) > 0
        self.assert_got(tx, "added_cargo", [resname, 40])

        cargo = self.get_ok(f"/station/{self.station}")["cargo"]
        res = self.assert_got(cargo, "resources", None)
        self.assert_got(res, resname, 40)
        self.assert_got(cargo, "usage", 30.0)

        cargobefore = self.get_ok(f"/station/{self.station}")["cargo"]
        tx = self.post_ok(f"/market/{self.station}/buy/carbon/5000")
        assert self.assert_got(tx, "added_cargo", None)[1] < 5000

    # Uses environment of previous test
    @functest
    def test_ship_upgrade(self):
        player = self.get_ok(f"/player/{self.id}")
        shipid = self.buy_a_ship()
        stationid = player["stations"][0]
        all_upg = self.get_ok(f"/station/{stationid}/shipyard/upgrade/{shipid}")
        for (upgr, data) in all_upg.items():
            assert self.assert_got(data, "price", None) > 100.0

        before = self.get_ok(f"/ship/{shipid}")

        got = self.post_ok(f"/station/{stationid}/shipyard/upgrade/{shipid}/cargoexpansion")
        assert self.assert_got(got, "cost", None) > 0

        got = self.post_ok(f"/station/{stationid}/shipyard/upgrade/{shipid}/reactorupgrade")
        assert self.assert_got(got, "cost", None) > 0

        got = self.post_ok(f"/station/{stationid}/shipyard/upgrade/{shipid}/hullupgrade")
        assert self.assert_got(got, "cost", None) > 0

        after = self.get_ok(f"/ship/{shipid}")
        assert after["cargo"]["capacity"] > before["cargo"]["capacity"]
        assert after["reactor_power"] > before["reactor_power"]
        assert after["hull_resistance"] > before["hull_resistance"]

    @functest
    def test_repair_refuel(self):
        player = self.create_test_player("test-rich-refuel-repair")
        shipid =  self.buy_a_ship()
        self.setup_crew(shipid)

        stationid = player["stations"][0]
        src = self.get_ok(f"/station/{stationid}")["position"]
        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": src[0]+30, "y": src[1]+30, "z": src[2]+30
        })
        self.tick_dur(cost["duration"])

        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": src[0], "y": src[1], "z": src[2]
        })
        self.tick_dur(cost["duration"])

        ship = self.get_ok(f"/ship/{shipid}")
        hull_decay = self.assert_got(ship, "hull_decay", None)
        hull_resistance = self.assert_got(ship, "hull_resistance", None)
        assert hull_decay > 0
        assert hull_decay < hull_resistance

        fuel = self.assert_got(ship, "fuel_tank", None)
        fuel_tank = self.assert_got(ship, "fuel_tank_capacity", None)
        assert fuel < fuel_tank
        assert fuel > 0

        before = (fuel, hull_decay)
        need = (fuel_tank - fuel, hull_decay)

        self.post_error(
            f"/station/{self.station}/refuel/{shipid}",
            errtype="NoFuelInCargo"
        )
        self.post_error(
            f"/station/{self.station}/repair/{shipid}",
            errtype="NoHullInCargo"
        )

        self.post_ok(f"/market/{self.station}/buy/fuel/{need[0] * 2}")
        got = self.post_ok(f"/station/{self.station}/refuel/{shipid}")
        self.assert_got(got, "added-fuel", need[0])

        self.post_ok(f"/market/{self.station}/buy/hull/{need[1] * 2}")
        got = self.post_ok(f"/station/{self.station}/repair/{shipid}")
        self.assert_got(got, "added-hull", need[1])

        ship = self.get_ok(f"/ship/{shipid}")
        self.assert_got(ship, "hull_decay", 0)
        self.assert_got(ship, "fuel_tank", self.assert_got(ship, "fuel_tank_capacity", None))

    @functest
    def test_upgrade_modules(self):
        player = self.create_test_player("test-rich-ship-mod-upgrade")
        shipid =  self.buy_a_ship()
        self.setup_crew(shipid)

        scan = self.post_ok(f"/station/{self.station}/scan")
        ship = self.get_ok(f"/ship/{shipid}")
        stationpos = ship["position"]
        distances = []
        for planet in self.assert_got(scan, "planets", None):
            distances.append((planet, compute_distance(planet["position"], ship["position"])))
        best = sorted(distances, key=lambda f: f[1])[0][0]
        if best["solid"]:
            mod = "miner"
        else:
            mod = "gassucker"

        got = self.post_ok(f"/station/{self.station}/shop/modules/{shipid}/buy/{mod}")
        modid = self.assert_got(got, "id", 1)
        operator = self.post_ok(f"/station/{self.station}/crew/hire/operator")
        opid = self.assert_got(operator, "id", None)
        self.post_ok(f"/station/{self.station}/crew/assign/{opid}/ship/{shipid}/{modid}")

        dest = best["position"]
        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        self.tick_dur(cost["duration"])

        ship = self.get_ok(f"/ship/{shipid}")
        self.assert_got(ship, "state", "Idle")
        self.addtrace("Ship arrived")
        old_extraction_rates = self.assert_got(self.post_ok(f"/ship/{shipid}/extraction/start"), "mining_rate", None)

        assert ("Carbon" in old_extraction_rates) or ("Hydrogen" in old_extraction_rates)
        self.tick()
        self.tick()
        self.post_ok(f"/ship/{shipid}/extraction/stop")

        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": stationpos[0], "y": stationpos[1], "z": stationpos[2]
        })
        self.tick_dur(cost["duration"])

        got = self.post_ok(f"/station/{self.station}/shop/modules/{shipid}/upgrade/1")
        self.assert_got(got, "cost", 4500)
        self.assert_got(got, "new-rank", 2)

        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        self.tick_dur(cost["duration"])

        new_extraction_rates = self.assert_got(self.post_ok(f"/ship/{shipid}/extraction/start"), "mining_rate", None)
        for res, rate in new_extraction_rates.items():
            assert rate > old_extraction_rates[res]
        self.test_crew_upg_extraction_rate = new_extraction_rates # For next test
        self.tick()
        self.tick()
        self.post_ok(f"/ship/{shipid}/extraction/stop")

        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": stationpos[0], "y": stationpos[1], "z": stationpos[2]
        })
        self.tick_dur(cost["duration"])

    # Uses environment from previous test
    @functest
    def test_buy_crew_upgrade(self):
        player = self.get_ok(f"/player/{self.id}")
        ship = player["ships"][0]
        shipid = ship["id"]
        shippos = ship["position"]
        dest = [shippos[0] + 30, shippos[1] + 30, shippos[2] + 30]
        beforewages = player["costs"]

        tx = self.post_ok(f"/market/{self.station}/buy/carbon/50")
        oldfee = self.assert_got(tx, "fees", None) / self.assert_got(tx, "removed_money", None)

        cost = self.post_ok("/calc/navigation", body={
            "ship_stats": ship["stats"],
            "origin": ship["position"],
            "dest": dest,
        })["costs"]
        oldt = self.assert_got(cost, "duration", None)

        prices = self.get_ok(f"/station/{self.station}/crew/upgrade/ship/{shipid}")
        for cid, data in prices.items():
            nextrk = self.assert_got(data, "rank", 2)
            price = self.assert_got(data, "price", None)
            mtype = self.assert_got(data, "member-type", None)

            got = self.post_ok(f"/station/{self.station}/crew/upgrade/ship/{shipid}/{cid}")
            nrank = self.assert_got(got, "new-rank", nextrk)
            cost = self.assert_got(got, "cost", price)

            new_prices = self.get_ok(f"/station/{self.station}/crew/upgrade/ship/{shipid}")
            assert new_prices[cid]["price"] > price
            assert new_prices[cid]["rank"] == (nextrk + 1)

            player = self.get_ok(f"/player/{self.id}")
            afterwages = player["costs"]
            assert afterwages > beforewages
            beforewages = afterwages

        station = self.get_ok(f"/station/{self.station}")
        trader = list(station["crew"].keys())[0]
        got = self.post_ok(f"/station/{self.station}/crew/upgrade/{trader}")
        self.assert_got(got, "new-rank", 2)
        self.assert_got(got, "cost", None)
        player = self.get_ok(f"/player/{self.id}")
        afterwages = player["costs"]
        assert afterwages > beforewages

        tx = self.post_ok(f"/market/{self.station}/buy/carbon/50")
        newfee = self.assert_got(tx, "fees", None) / self.assert_got(tx, "removed_money", None)
        self.addtrace("New fee", newfee, "< old fee", oldfee)
        assert newfee < oldfee

        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        newt = self.assert_got(cost, "duration", None)
        self.addtrace("New travel duration", newt, "< old travel duration", oldt)
        assert newt < oldt
        self.tick_dur(cost["duration"])

        need_solid = list(ship["modules"].values())[0]["modtype"] == "Miner"
        scan = self.post_ok(f"/station/{self.station}/scan")
        distances = []
        for planet in self.assert_got(scan, "planets", None):
            if planet["solid"] != need_solid:
                continue
            distances.append((planet, compute_distance(planet["position"], ship["position"])))
        best = sorted(distances, key=lambda f: f[1])[0][0]

        dest = best["position"]
        cost = self.post_ok(f"/ship/{shipid}/navigate", body={
            "x": dest[0], "y": dest[1], "z": dest[2]
        })
        self.tick_dur(cost["duration"])

        new_extraction_rates = self.assert_got(self.post_ok(f"/ship/{shipid}/extraction/start"), "mining_rate", None)
        for res, rate in self.test_crew_upg_extraction_rate.items():
            self.addtrace("resource", res, "new extraction rate", new_extraction_rates[res], ">", rate)
            assert new_extraction_rates[res] > rate

    @functest
    def test_industry(self):
        player = self.create_test_player("test-rich-industry")
        money = self.get_ok(f"/player/{self.id}")["money"]
        sta = self.station

        trader = self.post_ok(f"/station/{sta}/crew/hire/trader")
        traderid = self.assert_got(trader, "id", None)
        self.post_ok(f"/station/{sta}/crew/assign/{traderid}/trading")

        list_industry = self.get_ok(f"/station/{sta}/industry/buy")
        self.post_error(f"/station/{sta}/industry/buy/toto", errtype="InvalidArgument(\"industry_type\")")
        got = self.post_ok(f"/station/{sta}/industry/buy/simplefuelrefinery")
        new_money = self.get_ok(f"/player/{self.id}")["money"]
        assert new_money == (money - got["cost"])
        iid = got["id"]

        got = self.post_ok(f"/station/{sta}/crew/hire/operator")
        opid = got["id"]
        self.post_ok(f"/station/{sta}/crew/assign/{opid}/industry/{iid}")

        info = self.get_ok(f"/station/{sta}/industry/production/{iid}")

        for input in self.assert_got(info, "inputs", None):
            res = input[0]
            amnt = round(input[1], 5)
            tx = self.post_ok(f"/market/{sta}/buy/{res}/{amnt}")

        station = self.get_ok(f"/station/{self.station}")
        cargo = station["cargo"]["resources"]
        for input in self.assert_got(info, "inputs", None):
            self.assert_cmpf(cargo[input[0]], input[1])

        self.tick_dur(5)

        station = self.get_ok(f"/station/{self.station}")
        cargo = station["cargo"]["resources"]
        for input in self.assert_got(info, "inputs", None):
            self.assert_cmpf(cargo[input[0]], input[1])

        self.post_error(f"/station/{sta}/industry/start/1", errtype="NoSuchIndustryUnit")
        self.post_ok(f"/station/{sta}/industry/start/{iid}")

        stop = False
        while not stop:
            last = cargo
            self.tick()
            station = self.get_ok(f"/station/{self.station}")
            cargo = station["cargo"]["resources"]
            min = 100
            for input in self.assert_got(info, "inputs", None):
                self.addtrace(input[0], cargo[input[0]], input[1])
                assert cargo[input[0]] < input[1]
                if cargo[input[0]] == last[input[0]]:
                    stop = True
                    break

        station = self.get_ok(f"/station/{self.station}")
        cargo = station["cargo"]["resources"]
        for input in self.assert_got(info, "inputs", None):
            self.assert_cmpf(cargo[input[0]], 0.0)
        for output in self.assert_got(info, "outputs", None):
            self.assert_cmpf(cargo[output[0]], output[1])

    @functest
    def test_compute_travel_costs(self):
        scosts = self.post_ok("/navigation", body={
             "ship_stats": {
                 "speed": 1.0,
                 "fuel_consumption": 1.0,
                 "hull_usage_rate": 1.0,
             },
             "origin": (0, 0, 0),
             "dest": (1, 1, 1),
         })
        dcosts = self.post_ok("/navigation", body={
             "ship_stats": {
                 "speed": 2.0,
                 "fuel_consumption": 2.0,
                 "hull_usage_rate": 2.0,
             },
             "origin": (0, 0, 0),
             "dest": (1, 1, 1),
         })
        fcosts = self.post_ok("/navigation", body={
             "ship_stats": {
                 "speed": 2.0,
                 "fuel_consumption": 2.0,
                 "hull_usage_rate": 2.0,
             },
             "origin": (0, 0, 0),
             "dest": (2, 2, 2),
         })
        self.assert_got(scosts, "direction",
            self.assert_got(dcosts, "direction",
                self.assert_got(fcosts, "direction", None)
            )
        )
        self.assert_got(scosts, "distance",
            self.assert_got(dcosts, "distance",
                self.assert_got(fcosts, "distance", None) / 2
            )
        )
        self.assert_got(scosts, "duration",
            self.assert_got(dcosts, "duration",
                self.assert_got(fcosts, "duration", None) * 2
            ) / 2
        )
        self.assert_got(scosts, "fuel_consumption",
            self.assert_got(dcosts, "fuel_consumption",
                self.assert_got(fcosts, "fuel_consumption", None) / 2
            ) / 2
        )
        self.assert_got(scosts, "hull_usage",
            self.assert_got(dcosts, "hull_usage",
                self.assert_got(fcosts, "hull_usage", None) / 2
            ) / 2
        )

    @functest
    def test_compute_mining_rate(self):
        # TODO Functest computation mining rate
        pass

    @functest
    def test_compute_upgrade(self):
        # TODO Functest computation upgrade
        pass

def compute_distance(a, b):
    sum = 0
    sum += (b[0] - a[0]) ** 2
    sum += (b[1] - a[1]) ** 2
    sum += (b[2] - a[2]) ** 2
    return math.sqrt(sum)

if __name__ == "__main__":
    t = Tester(sys.argv[1], sys.argv[2])
    alltests = TESTS
    if len(sys.argv) >= 4:
        alltests = sys.argv[3:]
    nok = 0
    nerrors = 0
    for test in alltests:
        try:
            getattr(t, test)()
            nok += 1
        except AssertionError as exc:
            t.disp_error(exc)
            nerrors += 1
            if FAILFAST:
                break;
    if FAILFAST and nerrors > 0:
        print("")
        print("")
        print("FAIL FAST")
        print("")
    else:
        print(f"\nAll tests finished, {nok} OK, {nerrors} ERR\n")

    for (key, errdata) in t.saved_errors.items():
        side = (50 - len(key)) // 2
        print("*" * side, key, "*" * side )
        print(errdata)
        print("*" * 52)
        print("")

    sys.exit(nerrors > 0)
