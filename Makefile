.PHONY: build run

OUT=target/debug/libdwr.so

run: $(OUT)
	lua examples/main.lua

build: $(OUT)

$(OUT): src/**
	cargo build --lib
