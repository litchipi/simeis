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

    # Exercice:    Tester les additions
    assert x + y == y + x  # commutativité
    assert (x + y) + z == x + (y + z)  # associativité
    assert x + 0 == x  # élément neutre
    assert x + y >= x and x + y >= y  # croissance (opérandes positifs)


def distance():
    x1 = random.randrange(-100, 100)
    y1 = random.randrange(-100, 100)
    z1 = random.randrange(-100, 100)
    a = (x1, y1, z1)

    x2 = random.randrange(-100, 100)
    y2 = random.randrange(-100, 100)
    z2 = random.randrange(-100, 100)
    b = (x2, y2, z2)

    # Exercice:     Tester la distance entre le point A et le point B
    d_ab = get_dist(a, b)
    d_ba = get_dist(b, a)
    assert d_ab >= 0  # une distance est toujours positive ou nulle
    assert abs(d_ab - d_ba) < 1e-9  # symétrie: dist(a, b) == dist(b, a)
    assert get_dist(a, a) == 0  # distance d'un point à lui-même est nulle
    if a == b:
        assert d_ab == 0
    else:
        assert d_ab > 0


create_property_based_test(addition, time_test=3)
create_property_based_test(distance, regressions=[4480881574280375424], time_test=10)
