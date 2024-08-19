lint:
	cargo clippy --all --fix
	cargo fmt --all 
	nixfmt ./nix/*.nix 
	statix fix ./nix
	deadnix --edit --no-underscore
	git diff

build:
	cargo build --release

check:
	nixfmt -cv ./nix
	statix check ./nix
	deadnix --fail --no-underscore
	cargo fmt --all -- --check
	cargo test 
	cargo clippy -- -D warnings

