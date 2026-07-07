BIN_NAME=simeis-server
MANUAL=doc/manual.typ
MANUAL_OUT=doc/manual.pdf

.PHONY: all release strip manual check test tarpaulin clean dev

ifeq ($(OS),Windows_NT)
RELEASE_CMD=set "RUSTFLAGS=-C code-model=kernel -C codegen-units=1" && cargo build --release -p simeis-server
CLEAN_MANUAL=if exist "$(subst /,\,$(MANUAL_OUT))" del /Q "$(subst /,\,$(MANUAL_OUT))"
BIN_EXT=.exe
STRIP_CMD=llvm-strip
else
RELEASE_CMD=RUSTFLAGS="-C code-model=kernel -C codegen-units=1" cargo build --release -p simeis-server
CLEAN_MANUAL=rm -f "$(MANUAL_OUT)"
BIN_EXT=
STRIP_CMD=strip
endif

all: strip manual

release:
	$(RELEASE_CMD)

strip: release
	$(STRIP_CMD) target/release/$(BIN_NAME)$(BIN_EXT)

manual:
	typst compile $(MANUAL) $(MANUAL_OUT)

check:
	cargo check --workspace

test:
	cargo test --workspace

tarpaulin:
	cargo tarpaulin --workspace --out xml

clean:
	cargo clean
	$(CLEAN_MANUAL)

dev:
	cargo build -p simeis-server
