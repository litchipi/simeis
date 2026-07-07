import sys
import time
import random


def create_property_based_test(f, regressions=[], time_test=10):
    tstart = time.time()
    i = 0
    while (time.time() - tstart) < time_test:
        if i < len(regressions):
            seed = regressions[i]
        else:
            seed = random.randrange(0, 2**64)
        random.seed(seed)
        try:
            f()
            print("Test", f.__name__, i, "OK")
        except AssertionError as err:
            print("Test", f.__name__, "failed with seed", seed)
            print(err)
            sys.exit(1)
        i += 1


### Example
import math


def get_dist(a, b):
    return math.sqrt(((a[0] - b[0]) ** 2) + ((a[1] - b[1]) ** 2) + ((a[2] - b[2]) ** 2))


def addition():
    x = random.randrange(0, 10000)
    y = random.randrange(0, 10000)
    z = random.randrange(0, 10000)

    assert x + y == y + x, f"Commutativité échouée : {x} + {y} != {y} + {x}"

    assert (x + y) + z == x + (
        y + z
    ), f"Associativité échouée : ({x}+{y})+{z} != {x}+({y}+{z})"

    assert x + 0 == x, f"Élément neutre échoué : {x} + 0 != {x}"

    assert x + y >= x, f"Croissance échouée : {x} + {y} < {x}"
    assert x + y >= y, f"Croissance échouée : {x} + {y} < {y}"


def distance():
    x1 = random.randrange(-100, 100)
    y1 = random.randrange(-100, 100)
    z1 = random.randrange(-100, 100)
    a = (x1, y1, z1)

    x2 = random.randrange(-100, 100)
    y2 = random.randrange(-100, 100)
    z2 = random.randrange(-100, 100)
    b = (x2, y2, z2)

    epsilon = 1e-9

    assert get_dist(a, b) >= 0, f"Non-négativité échouée : dist({a}, {b}) < 0"

    assert get_dist(a, a) < epsilon, f"Identité échouée : dist({a}, {a}) != 0"
    assert get_dist(b, b) < epsilon, f"Identité échouée : dist({b}, {b}) != 0"

    assert (
        abs(get_dist(a, b) - get_dist(b, a)) < epsilon
    ), f"Symétrie échouée : dist({a},{b}) != dist({b},{a})"

    expected = math.sqrt((x1 - x2) ** 2 + (y1 - y2) ** 2 + (z1 - z2) ** 2)
    assert (
        abs(get_dist(a, b) - expected) < epsilon
    ), f"Distance incorrecte : get_dist={get_dist(a, b)}, attendu={expected}"

    x3 = random.randrange(-100, 100)
    y3 = random.randrange(-100, 100)
    z3 = random.randrange(-100, 100)
    c = (x3, y3, z3)
    assert (
        get_dist(a, b) <= get_dist(a, c) + get_dist(c, b) + epsilon
    ), f"Inégalité triangulaire échouée : dist({a},{b}) > dist({a},{c}) + dist({c},{b})"


heavy = "--heavy" in sys.argv
time_addition = 300 if heavy else 3
time_distance = 300 if heavy else 10

create_property_based_test(addition, time_test=time_addition)
create_property_based_test(
    distance, regressions=[4480881574280375424], time_test=time_distance
)
