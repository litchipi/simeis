.PHONY: build tests check clean modify-typ

build:
	RUSTFLAGS="-C code-model=kernel -C codegen-units=1" cargo build --verbose

build-release:
	RUSTFLAGS="-C code-model=kernel -C codegen-units=1" cargo build --release


clean : 
	cargo clean

tests : 
	cargo test

update :
	cargo update

full-check : check format-check smell-check
	@echo "Check done"

check : 
	cargo check

format-check :
	cargo fmt --check

smell-check :
	cargo clippy



strip : build
	strip target/debug/simeis-server.exe

strip-release : build-release
	strip target/release/simeis-server.exe


modify-typ : 
	typst compile doc/manual.typ doc/manual.pdf