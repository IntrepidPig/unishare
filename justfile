export RUST_LOG := "info,server=trace,client=trace,unishare_transport=trace"
#export RUST_LOG := "trace"

build:
	cargo build

run-example EXAMPLE *ARGS:
	cargo run --example {{EXAMPLE}} {{ARGS}}

debug-example EXAMPLE *ARGS:
	cargo build --example {{EXAMPLE}}
	rust-gdb -ex 'run' --args ./target/debug/examples/{{EXAMPLE}} {{ARGS}}

test:
	cargo test --tests

test-sync:
	#!/bin/bash
	cd transport
	cargo test --tests -- --nocapture --test-threads 1

start-test-server:
	cargo run --bin server -- -p 2048 ./storage/test.storage

start-test-client:
	sudo umount ./client/mnt || true
	cargo run --bin client -- localhost:2048 ./client/mnt
	sudo umount ./client/mnt

debug-test-client:
	sudo umount ./client/mnt || true
	cargo build --bin client
	rust-gdb -ex 'run' --args ./target/debug/client localhost:2048 ./client/mnt
	sudo umount ./client/mnt

clean-test-data:
	sudo umount ./client/mnt || true
	rm -r ./storage/test.storage