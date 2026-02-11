.PHONY: build run

OUT=target/debug/libdwr.so

run: $(OUT)
	luajit examples/main.lua

rust:
	cargo run

build: $(OUT)

$(OUT): src/**
	cargo build --lib
