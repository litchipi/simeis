.PHONY: build tests check clean modify-typ

strip : build
	strip target/debug/simeis-server.exe

build:
	RUSTFLAGS="-C code-model=kernel -C codegen-units=1" cargo build --verbose

build-release:
	RUSTFLAGS="-C code-model=kernel -C codegen-units=1" cargo build --release

tests : 
	cargo test

check : 
	cargo check

clean : 
	cargo clean

modify-typ : 
	typst compile doc/manual.typ doc/manual.pdf

update :
	cargo update