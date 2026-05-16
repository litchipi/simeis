import os
import json

PYSDK_IGNORE_CHECKS = {
    "post": [
        "/station/{station_id}/crew/assign/{crew_id}/ship/{ship_id}/pilot"
    ],
    "get": [
    ],
}
RSSDK_IGNORE_CHECKS = {
    "post": [],
    "get": [],
}

def get_inner_parenth(s):
    return ")".join("(".join(s.split("(")[1:]).split(")")[:-1])

def replace_params(fr, to):
    if fr.count("{") != to.count("{"):
        return None
    params = [s.split("}")[0] for s in fr.split("{")[1:]]
    if len(params) == 0:
        return to
    idx = 0
    start = to.find("{")
    result = to[:start]
    for param in params:
        end = to.find("}", start) + 1
        result += "{" + param + "}"
        start = to.find("{", end)
        if start == -1:
            start = len(to)
        result += to[end:start]
    return result

def find_next_post(s, start):
    n1 = s.find("self.post(", start)
    n2 = s.find("self.post_with_body(", start)
    if n1 < 0 and n2 < 0:
        return None, None
    if n1 >= 0:
        n = n1
        n1 += len("self.post(")
    if n2 >= 0:
        if n1 == -1:
            n = n2
        else:
            n = min(n1, n2)
        n2 += len("self.post_with_body(")
    # print(n1, n2, n)
    start = s.find("\"", n) + 1
    assert start > 0 # -1 + 1 becomes 0
    end = s.find("\"", start)
    p = s[start:end]
    return p, end

def find_next_get(s, start):
    n = s.find("self.get(", start)
    if n < 0:
        return None, None
    n += len("self.get(")
    start = s.find("\"", n) + 1
    assert start > 0 # -1 + 1 becomes 0
    end = s.find("\"", start)
    p = s[start:end]
    return p, end

def match_a_path(s, paths, method):
    np = s.count("{")
    for checkpath, data in swagger["paths"].items():
        if method not in data.keys():
            continue
        if checkpath.count("{") != np:
            continue
        if replace_params(checkpath, s) == checkpath:
            return checkpath
    return None

def check_missing_generic(sdkcode, paths, ign):
    done = {
        "get": {p:False for p in paths if "get" in paths[p]},
        "post": {p:False for p in paths if "post" in paths[p]},
    }
    next_get, idx = find_next_get(sdkcode, 0)
    while next_get is not None:
        # print("GET", next_get, idx)
        matched = match_a_path(next_get, paths, "get")
        if matched is not None:
            done["get"][matched] = True
        next_get, idx = find_next_get(sdkcode, idx)

    next_post, idx = find_next_post(sdkcode, 0)
    while next_post is not None:
        # print("POST", next_post, idx)
        matched = match_a_path(next_post, paths, "post")
        if matched is not None:
            done["post"][matched] = True
        next_post, idx = find_next_post(sdkcode, idx)

    missing = []
    for m, data in done.items():
        for p, d in data.items():
            if not d and p not in ign[m]:
                missing.append((m, p))
    return missing

def check_missing_python_sdk(root, swagger):
    with open(os.path.join(root, "sdk/python.py"), "r") as f:
        pysdk = f.read()
    return check_missing_generic(pysdk, swagger["paths"], PYSDK_IGNORE_CHECKS)


def check_missing_rust_sdk(root, swagger):
    with open(os.path.join(root, "sdk/rust/src/lib.rs"), "r") as f:
        rssdk = f.read()
    return check_missing_generic(rssdk, swagger["paths"], RSSDK_IGNORE_CHECKS)

root = "/home/john/work/perso/games/simeis"
with open(os.path.join(root, "doc/swagger.json"), "r") as f:
    swagger = json.load(f)

print("PY")
missing = check_missing_python_sdk(root, swagger)
for m, p in missing:
    print("Missing", m, p)

print("")
print("RS")
missing = check_missing_rust_sdk(root, swagger)
for m, p in missing:
    print("Missing", m, p)
