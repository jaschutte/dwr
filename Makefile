.PHONY: build run

OUT=target/debug/libdwr.so

run: $(OUT)
	nixGL luajit examples/main.lua

rust:
	nixGL cargo run

build: $(OUT)

$(OUT): src/**
	cargo build --lib
