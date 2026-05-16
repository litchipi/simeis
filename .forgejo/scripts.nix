{ pkgs, deps }: let
  python = pkgs.python312.withPackages (p: [ p.requests ]);
  compile_and_run = {
    key,
    exec,
    features ? [],
    release ? false,
    head ? "",
  }: let
    fflags = "--features ${builtins.concatStringsSep "," (features ++ ["tokio"])}";
    rf = if release then "--release" else "";
    bcmd = "cargo build ${rf} --target-dir ./target/${key} ${fflags} --no-default-features";
    targetdir = "./target/${key}/${if release then "release" else "debug"}";
    rcmd = "${targetdir}/simeis-server 1>/tmp/${key}_simeis_logs 2>&1 &";
  in ''
    #!/bin/bash
    export CARGO_HOME=$PWD/.cargohome
    export RUST_BACKTRACE=full
    export RUST_LOG=debug
    mkdir -p target/${key}
    ${head}
    echo "[*] Building server"
    ${bcmd}
    echo "[*] Starting the server"
    ${rcmd}
    sleep 2
    if [ -z "$(jobs -r)" ]; then
      echo "!!! Failed to start the server";
      cat /tmp/${key}_simeis_logs
      exit 1;
    fi

    echo "[*] Executing the command for ${key}"
    if ! ${exec}; then
      echo "!!! Error occured while executing ${key}"
      kill "$(jobs -p)"
      echo "Server logs:"
      tail -n 15 /tmp/${key}_simeis_logs
      exit 1;
    fi

    echo "[*] Killing server"
    kill "$(jobs -p)"
    echo "[*] Finished"
  '';
in {
  prepare_class = let
    app = pkgs.writeShellApplication {
      name = "prepare-class";
      runtimeInputs = [];
      text = ''
        if ! [ -z "$(git diff)" ]; then
          echo "Staging changes, commit then execute again"
          exit 1
        fi

        git checkout -B class
        rm -rf .forgejo
        rm flake.*
        rm economy.ods
        rm example/bigtest.sh
        rm -rf .swagger
        git add .
        git commit -m "Preparing project for class"
        git push -f -u origin class
      '';
    };
  in "${app}/bin/prepare-class";

  unit_tests = let
    app = pkgs.writeShellApplication {
      name = "unit-tests";
      runtimeInputs = deps;
      text = ''
        export CARGO_HOME=$PWD/.cargohome
        cargo test
      '';
    };
  in "${app}/bin/unit-tests";

  functional_tests = let
    app = pkgs.writeShellApplication {
      name = "functionnal-tests";
      runtimeInputs = deps ++ [ python ];
      text = compile_and_run {
        key = "functests";
        features = ["testing"];
        exec = "python3 .forgejo/functests.py 127.0.0.1 9345 \"$@\"";
      };
    };
  in "${app}/bin/functionnal-tests";

  check_rust_code = let
    allIgnoredCVE = [
      "RUSTSEC-2025-0047"
      "RUSTSEC-2026-0097"
      "RUSTSEC-2024-0436"  # To look, paste is required by compio, but would be nice to upgrade
    ];
    allIgnored = builtins.concatStringsSep " " (builtins.map (s: "--ignore ${s}") allIgnoredCVE);
    app = pkgs.writeShellApplication {
      name = "rust-check-code";
      runtimeInputs = deps ++ [ pkgs.gcc ];

      text = ''
        set -e
        export CARGO_HOME=$PWD/.cargohome
        echo "[*] Basic checks on the project"
        cargo check
        cargo check --features testing,extraspeed,tokio --no-default-features
        cargo clippy --no-deps --frozen -- -D warnings
        cargo fmt --check

        echo "[*] Checking the SDK"
        rm -rf /tmp/rust_sdk
        cp -r sdk/rust/ /tmp/rust_sdk
        cd /tmp/rust_sdk
        cargo check
        cargo clippy --no-deps --frozen -- -D warnings
        cargo fmt --check
        cd -

        echo "[*] Security audit on the project"
        cargo audit ${allIgnored}
        if ! [ -d supply-chain ]; then
          cargo vet init
        fi
        cargo vet
      '';
    };
  in "${app}/bin/rust-check-code";

  check_todos = let
    ignored=builtins.concatStringsSep "|" [
      ".git"
      ".forgejo/scripts.nix"
      ".forgejo/workflows"
      ".swagger"
      "TODO.md"
    ];

    app = pkgs.writeShellApplication {
      name = "check-todos";
      runtimeInputs = [ pkgs.ripgrep pkgs.jq ];
      text = ''
        set -e
        ERRCODE=0
        CURL_OPTS="-s"
        git fetch origin "$GITHUB_BASE_REF"
        echo ""

        set +e
        FILES_TO_CHECK=$( rg -l "TODO" | rg -v "${ignored}" | tr '\n' ' ')
        git diff "origin/$GITHUB_BASE_REF" -- "$FILES_TO_CHECK" \
          | rg -v "^-" \
          | tee todos_to_check

        rg -o "\(#\d+\)" todos_to_check \
          | rg -o "\d+" \
          | sort -u -n \
          | tee issues_to_check

        while read -r ISSUE; do
          if ! curl "$CURL_OPTS" \
            "$GITHUB_SERVER_URL/api/v1/repos/$GITHUB_REPOSITORY/issues/$ISSUE" \
            | jq ".state" \
            | rg "\"open\"" \
            2>/dev/null 1>/dev/null
          then
            echo "[!] Issue $ISSUE is closed or missing"
            ERRCODE=1
          fi
        done < issues_to_check
        echo ""

        if echo "$FILES_TO_CHECK" | xargs -n 1 rg "TODO" | rg -v "\(#\d+\)"; then
          echo "[!] Some TODOs are not linked to an existing issue"
          ERRCODE=1
        fi
        echo ""

        exit $ERRCODE
      '';
    };
    in "${app}/bin/check-todos";

    api_sanity = let
      app = pkgs.writeShellApplication {
        name = "api_sanity";
        runtimeInputs = deps ++ [ python pkgs.ripgrep ];
        text = compile_and_run {
          key = "apisanity";
          features = ["testing"];
          exec = "python3 .forgejo/check_api_sanity.py 127.0.0.1 9345 \"$@\"";
        };
      };
    in "${app}/bin/api_sanity";

    client_run_well = let
      app = pkgs.writeShellApplication {
        name = "client-run-well";
        runtimeInputs = deps ++ [ pkgs.ripgrep pkgs.gawk python ];
        text = compile_and_run {
          key = "client_runwell";
          features = ["extraspeed"];
          release=true;
          exec = "start_client_test";
          head = ''
            #!/bin/bash
            function start_client_test() {
              python3 example/python/client.py ci 127.0.0.1 8080 1>/tmp/client_runwell_clilogs 2>&1 &
              client_pid=$!
              sleep 3
              if ! jobs -p |rg "$client_pid" 1>/dev/null; then
                echo "Client failed to start"
                return 1
              fi
              echo "[*] Client started"
              tstart=$(date +%s)
              while [ $(($(date +%s) - tstart)) -lt 600 ]; do
                TBL=$(cat /tmp/client_runwell_clilogs|rg "time left before lost"|tail -n 1|awk '{print $11}')
                if [ -z "$TBL" ]; then
                  sleep 1
                  continue;
                fi
                echo "Time before lost: $TBL"
                if [ "$TBL" -gt 5000 ]; then
                  echo "Reached 5000 secs before lost, considering it a success"
                  break
                fi
                sleep 5
              done
              echo "[*] Test finished, killing client..."
              kill "$client_pid"
              wait "$client_pid"
              echo "[*] Client stopped"
            }
          '';
        };
      };
    in "${app}/bin/client-run-well";

    check_sdk_sync = let
      ignored = builtins.concatStringsSep "|" [
        "__init__"
        " api\\("
        " get\\("
        " post\\("
      ];
      app = pkgs.writeShellApplication {
        name = "check-sdk-sync";
        runtimeInputs = [ pkgs.ripgrep pkgs.gawk pkgs.gnused pkgs.diffutils ];
        text = ''
          cat sdk/python.py \
          | rg "def.*\(self" \
          | rg -v "${ignored}" \
          | awk -F '(' '{print $1}' \
          | awk '{print $NF}' \
          | rg -v "${ignored}" \
          | sort -u > pymethods

          cat sdk/rust/src/lib.rs \
          | tr '\n' ' ' \
          | rg -o 'fn(\w|\s)*?\(\s*&(mut)?\s*self' \
          | rg -v "${ignored}" \
          | awk -F '(' '{print $1}' \
          | awk '{print $NF}' \
          | sed 's/assign_crew_to_ship_module/assign_crew_to_ship/' \
          | sed 's/assign_crew_as_ship_pilot/assign_crew_to_ship/' \
          | sort -u > rsmethods

          if ! diff pymethods rsmethods; then
            echo "Python SDK and Rust SDK are not in sync"
            exit 1
          fi

          echo "Python SDK and Rust SDK are in sync"
        '';
      };
    in "${app}/bin/check-sdk-sync";
  }
