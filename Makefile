all: core

core:
	cd core && cargo build

test: core
	cd python && python3 tests.py

.PHONY: core test
