import sys
import time
import random
import argparse
import math


def create_property_based_test(f, regressions=None, time_test=10):
    if regressions is None:
        regressions = []
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


def get_dist(a, b):
    return math.sqrt(((a[0] - b[0]) ** 2) + ((a[1] - b[1]) ** 2) + ((a[2] - b[2]) ** 2))


def addition():
    x = random.randrange(0, 10000)
    y = random.randrange(0, 10000)
    z = random.randrange(0, 10000)

    # Commutativity
    assert (x + y) == (y + x)

    # Associativity
    assert (x + y) + z == x + (y + z)

    # Identity
    assert (x + 0) == x


def distance():
    x1 = random.randrange(-100, 100)
    y1 = random.randrange(-100, 100)
    z1 = random.randrange(-100, 100)
    a = (x1, y1, z1)

    x2 = random.randrange(-100, 100)
    y2 = random.randrange(-100, 100)
    z2 = random.randrange(-100, 100)
    b = (x2, y2, z2)

    d1 = get_dist(a, b)
    d2 = get_dist(b, a)

    # Symmetry
    assert abs(d1 - d2) < 1e-12

    # Non-negativity
    assert d1 >= 0.0

    # Identity
    assert get_dist(a, a) == 0.0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--time", type=float, default=10.0, help="seconds per property test")
    parser.add_argument("--regression", type=int, action='append', help="regression seeds to replay")
    args = parser.parse_args()

    create_property_based_test(addition, regressions=args.regression or [], time_test=args.time)
    create_property_based_test(distance, regressions=args.regression or [], time_test=args.time)


if __name__ == '__main__':
    main()
