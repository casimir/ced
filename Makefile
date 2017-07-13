all: core

core:
	cd core && cargo build

test: core
	cd python && env CED_BIN_PATH=../core/target/debug/ced-core python -m unittest

.PHONY: core
