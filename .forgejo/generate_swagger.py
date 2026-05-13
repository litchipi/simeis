import os
import sys
import json
import requests

IGNORED_LINES = [
    "TO" + "DO",
    "FIXME",
]

MDATA_KEYS = ["summary", "returns", "postbody"]
OPT_KEYS = ["postbody"]

def check_all_metadata(trace, mdata):
    for k in MDATA_KEYS:
        if k not in mdata and k not in OPT_KEYS:
            print("")
            print(f"ERROR: Metadata from {trace} is missing key {k}")
            print("")
            sys.exit(1)

def get_url_params(path):
    idx = 0
    result = []
    while "{" in path[idx:]:
        next_idx = idx + 1 + path[idx:].find("{")
        assert next_idx >= 0
        idx_end = next_idx + path[next_idx:].find("}")
        assert idx_end >= 0
        param = path[next_idx:idx_end]
        result.append(param)
        idx = idx_end + 1
    return result

def get_metadata(comments):
    mdata = {}
    doc = ""
    for commentraw in comments:
        comment = commentraw.removeprefix("//").strip()
        ismdata = False
        for key in MDATA_KEYS:
            if "@" + key in comment:
                mdata[key] = comment.removeprefix("@" + key).strip()
                ismdata = True
        if not ismdata:
            doc += comment + "\n"
    return (mdata, doc.strip())

def get_version(cargo_toml_file):
    with open(cargo_toml_file, "r") as f:
        cargotoml = f.read()
    for line in cargotoml.split("\n"):
        if "version" in line:
            return line.split("=")[1].strip().strip('"')
    raise Exception("Version not found in cargo.toml")

def get_comments_before(data, tag):
    result = []
    for (nline, line) in enumerate(data):
        line = line.strip()
        if tag in line:
            if any(["@noswagger" in l for l in result]):
                result = []
                continue
            mdata, doc = get_metadata(result)
            return (nline, mdata, doc)
        elif line.startswith("//") and all([s not in line for s in IGNORED_LINES]):
            result.append(line)
        else:
            result = []
    return None

def mk_schema(val):
    if isinstance(val, dict):
        prop = {k: mk_schema(v) for k, v in val.items()}
        return {"type": "object", "properties": prop }
    elif isinstance(val, list):
        return { "type": "array", "items": mk_schema(val[0]) }
    elif isinstance(val, str):
        return { "type": "string" }
    elif isinstance(val, int):
        return { "type": "integer" }
    elif isinstance(val, float):
        return { "type": "number" }
    elif isinstance(val, bool):
        return { "type": "boolean" }
    else:
        ty = data.__class__.__name__
        raise Exception(f"Unsupported type for OpenAPI type: {ty}")

class ApiChecker:
    def __init__(self, host, port, root):
        self.host = host
        self.port = port
        self.root = root
        self.examples = {}
        self.path_params = {}
        self.bodies = {}
        version = get_version(os.path.join(root, "../Cargo.toml"))
        with open(os.path.join(root, "main.rs"), "r") as f:
            main = f.read().split("\n")
        _, mdata, description = get_comments_before(main, "#[ntex::main]")
        assert description is not None
        self.swagger = {
            "openapi": "3.0.4",
            "info": {
                "title": "Simeis",
                "description": description,
                "version": version,
            },
            "servers": {},
            "paths": {},
        }
        self.swagger["info"].update(mdata)
        assert self.get("/ping")["error"] == "ok"

    def get(self, path, timeout=5):
        headers = {}
        if hasattr(self, "key"):
            headers["Simeis-Key"] = str(self.key)
        url = f"http://{self.host}:{self.port}/{path}"
        got = requests.get(url, headers=headers, timeout=timeout)
        if got.status_code != 200:
            print("Got a status code", got.status_code, "on GET", path)
            sys.exit(1)
        return json.loads(got.text)

    def post(self, path, timeout=5, body=None):
        headers = {}
        if hasattr(self, "key"):
            headers["Simeis-Key"] = str(self.key)
        url = f"http://{self.host}:{self.port}/{path}"
        got = requests.post(url, headers=headers, timeout=timeout, json=body)
        if got.status_code != 200:
            print("Got a status code", got.status_code, "on POST", path)
            sys.exit(1)
        return json.loads(got.text)

    def get_example(self, method, path, params):
        method=method.upper()
        exkey = f"{method}:{path}"
        if exkey in self.examples:
            print("Pre-registered example")
            return self.examples[exkey]
        for param in params:
            if param not in self.path_params:
                print(f"URL parameter {param} of path {path} is not prepared")
                sys.exit(1)
            value = self.path_params[param]
            path = path.replace("{" + param + "}", str(value))
        if "{" in path:
            print(f"Path expansion is missing for {exkey}: {path}")
            sys.exit(1)
        if method.upper() == "GET":
            data = self.get(path)
        elif method.upper() == "POST":
            data =  self.post(path)
        else:
            print("unsupported method", method.upper(), "in examples fetching")
            sys.exit(1)
        if data["error"] != "ok":
            print(f"Error in example for {method}:{path}:", data["error"])
            sys.exit(1)
        return data

    def crawl(self):
        with open(os.path.join(self.root, "api.rs"), "r") as f:
            rootapi = [line for line in f.read().split("\n") if "::configure" in line]

        for section in rootapi:
            section_name = section.split("::configure")[0].strip()
            if section_name == "system":
                path = ""
            else:
                path = section.split("\"")[1]
            self.crawl_section(section_name, [path])

    def crawl_section(self, name, paths):
        print("Crawling {} (root path {})".format(name, "/".join(paths)))
        fpath = os.path.join(self.root, "api", name) + ".rs"
        with open(fpath, "r") as f:
            code = f.read().split("\n")

        self.crawl_for_method("get", name, code, "/".join(paths))
        self.crawl_for_method("post", name, code, "/".join(paths))

        for line in code:
            line = line.lstrip()
            if line.startswith(".configure(|srv|"):
                name = line.split("::configure(\"")[0].split("::")[-1]
                path = line.split("::configure(\"")[1].split("\"")[0].lstrip("/")
                self.crawl_section(name, paths + [ path ])

    def crawl_for_method(self, method, tag, code, rootpath):
        while True:
            got = get_comments_before(code, "#[web::" + method.lower())
            if got is None:
                return
            nline, mdata, doc = got
            path = rootpath + code[nline].split("\"")[1]
            all_params = get_url_params(path)
            name = code[nline+1].split("(")[0].split(" ")[-1]
            print("Found {} {} API {} at line {}".format(method.upper(), path, name, nline))
            exkey = f"{method.upper()}:{path}"
            check_all_metadata(exkey, mdata)
            example = self.get_example(method, path, all_params)
            data = {
                "description": doc,
                "responses": {
                    "200": {
                        "description": mdata.pop("returns"),
                        "content": {
                            "application/json": {
                                "example": example,
                            },
                        },
                    },
                },
                "tags": [ tag ],
                "parameters": [{
                    "name": n,
                    "in": "path",
                    "required": True,
                } for n in all_params]
            }
            if exkey in self.bodies:
                data["requestBody"] = {
                    "required": True,
                    "content": { "application/json": {
                        "schema": mk_schema(self.bodies[exkey]),
                        "example": self.bodies[exkey],
                    }},
                }
                if "postbody" in mdata:
                    data["requestBody"]["description"] = mdata.pop("postbody")
            data.update(mdata)
            self.swagger["paths"][path] = { method: data }
            code = code[nline+1:]

    def check_rust_sdk(self):
        pass

    def check_python_sdk(self):
        pass

    def check_func_tests(self):
        pass

    def generate_html_file(self, proj, outfile):
        with open(os.path.join(proj, ".swagger/swagger-ui.css"), "r") as f:
            swaggercss = f.read()
        with open(os.path.join(proj, ".swagger/swagger-ui-bundle.js"), "r") as f:
            swaggerbundle = f.read()
        with open(os.path.join(proj, ".swagger/swagger-ui-standalone-preset.js"), "r") as f:
            swaggerpreset = f.read()
        with open(os.path.join(proj, ".swagger/swagger-initializer.js"), "r") as f:
            swaggerinit = f.read()
        swaggerui = f"<style>{swaggercss}</style>"
        swaggerui += f"<script>{swaggerbundle}</script>"
        swaggerui += f"<script>{swaggerpreset}</script>"
        swaggerui += f"<script>{swaggerinit}</script>"
        swaggerui += "<div id=\"ui\"></div>"

        with open(outfile, "w") as f:
            f.write("<html><head>")
            f.write("<meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\">")
            f.write(f"</head><body>{swaggerui}</body></html>")
        pass

    def generate_swagger(self, outfile):
        with open(outfile, "w") as f:
            json.dump(self.swagger, f, indent=2)

    def prepare_path(self, path, method=None, show=True, body=None, **params):
        method=method.upper()
        exkey = f"{method}:{path}"
        if body is not None:
            self.bodies[exkey] = body
        self.path_params.update(params)
        for (key, val) in self.path_params.items():
            if "{" + key + "}" not in path:
                continue
            path = path.replace("{" + key + "}", str(val))
        if "{" in path:
            print(f"Missing path expansion for example preparation of {exkey}: {path}")
            sys.exit(1)
        if method == "GET":
            data = self.get(path)
        else:
            data = self.post(path, body=body)
        if data["error"] != "ok":
            print(f"Preparation of path {exkey} raised an error:", data["error"])
            sys.exit(1)
        self.examples[exkey] = data
        if show:
            print(data)
        return data

    def prepare_post(self, path, **kwargs):
        return self.prepare_path(path, method="POST", **kwargs)

    def prepare_get(self, path, **kwargs):
        return self.prepare_path(path, method="GET", **kwargs)

    def tick_for_event(self, want):
        while True:
            self.post("/tick")
            logs = self.prepare_get("/syslogs", show=False)
            for ev in logs["events"]:
                print(want, ev)
                if ev["type"].startswith(want):
                    print("Reached event", want)
                    return

    def prepare_examples(self):
        data = self.prepare_post("/player/new/{name}", name="test-rich-swagger")
        self.key = data["key"]
        pid = data["playerId"]
        player = self.prepare_get("/player/{player_id}", player_id=pid)
        station = self.prepare_get("/station/{station_id}", station_id=player["stations"][0])

        trader = self.prepare_post("/station/{station_id}/crew/hire/{crewtype}", crewtype="trader")
        self.prepare_post("/station/{station_id}/crew/fire/{crew_id}", crew_id=trader["id"])
        trader = self.prepare_post("/station/{station_id}/crew/hire/{crewtype}")
        self.prepare_post("/station/{station_id}/crew/assign/{crew_id}/trading", crew_id=trader["id"])
        self.prepare_post("/station/{station_id}/crew/upgrade/{crew_id}")
        self.prepare_post("/market/{station_id}/buy/{resource}/{amnt}", resource="Fuel", amnt=100)
        self.prepare_post("/market/{station_id}/buy/{resource}/{amnt}", resource="Hull", amnt=100)

        industry = self.prepare_post("/station/{station_id}/industry/buy/{name}", name="simplefuelrefinery")
        operator = self.prepare_post("/station/{station_id}/crew/hire/{crewtype}", crewtype="operator")
        self.prepare_post("/station/{station_id}/crew/assign/{crew_id}/industry/{industry_id}", crew_id=operator["id"], industry_id=industry["id"])

        planets = self.prepare_post("/station/{station_id}/scan")
        target = planets["planets"][0]["position"]

        allships = self.prepare_get("/station/{station_id}/shipyard/list")
        fship = allships["ships"][0]["id"]
        ship = self.prepare_post("/station/{station_id}/shipyard/buy/{ship_id}", ship_id=fship)
        self.prepare_post("/station/{station_id}/shipyard/upgrade/{ship_id}/{upgrade_type}", upgrade_type="reactorupgrade")
        self.prepare_post("/station/{station_id}/shop/cargo/buy/{amount}", amount=1)
        pilot = self.prepare_post("/station/{station_id}/crew/hire/{crewtype}", crewtype="pilot")
        self.prepare_post("/station/{station_id}/crew/assign/{crew_id}/ship/{ship_id}/pilot", crew_id=pilot["id"])
        mod_type = "Miner"
        resource = "Carbon"
        module = self.prepare_post("/station/{station_id}/shop/modules/{ship_id}/buy/{modtype}", modtype=mod_type)
        operator = self.prepare_post("/station/{station_id}/crew/hire/{crewtype}", crewtype="operator")
        self.prepare_post("/station/{station_id}/crew/assign/{crew_id}/ship/{ship_id}/{mod_id}", crew_id=operator["id"], mod_id=module["id"])

        self.prepare_post("/calc/navigation", body={
            "ship_stats": { "speed": 3.0, "fuel_consumption": 2.0, "hull_usage_rate": 1.0 },
            "origin": [0, 0, 0],
            "dest": [1, 2, 3],
        })
        cost = self.prepare_post("/ship/{ship_id}/navigate", body={
            "x": target[0],
            "y": target[1],
            "z": target[2]
        })
        self.tick_for_event("ShipFlightFinished")
        rate = self.prepare_post("/ship/{ship_id}/extraction/start")
        tfilled = rate["time_fill_cargo"] / 2.0
        for _ in range(int(tfilled / (20.0 / 1000.0))):
            self.post("/tick")
        rate = self.prepare_post("/ship/{ship_id}/extraction/stop")
        cost = self.prepare_post("/ship/{ship_id}/navigate", body={
             "x": station["position"][0],
             "y": station["position"][1],
             "z": station["position"][2],
        })
        self.tick_for_event("ShipFlightFinished")
        self.prepare_post("/ship/{ship_id}/unload/{station_id}/{resource}/{amnt}", resource=resource, amnt=1)

        pl = self.prepare_post("/player/new/{name}", name="xX_GigaProf_Xx")
        self.prepare_get("/player/{player_id}", player_id=pl["playerId"])

        self.prepare_post("/calc/mining_rate", body={
            "module_type": "Miner",
            "module_rank": 5,
            "operator_rank": 2,
            "planet_coord": target,
        })

        self.prepare_post("/calc/upgrade", body={
            "module_type": "Miner",
            "module_start_rank": 2,
            "module_end_rank": 4,
            "crew_start_rank": 1,
            "crew_end_rank": 3,
        })

ip = sys.argv[1]
port = sys.argv[2]

proj = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
rootd = os.path.join(proj, "simeis-server/src")

checker = ApiChecker(ip, port, rootd)
checker.prepare_examples()
checker.crawl()
checker.check_rust_sdk()
checker.check_python_sdk()
checker.check_func_tests()
checker.generate_swagger(os.path.join(proj, "doc/swagger.json"))
checker.generate_html_file(proj, os.path.join(proj, "doc/swagger-ui.html"))
