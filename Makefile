all: core

core:
	cd core && cargo build

test: core
	cd python && python tests.py

.PHONY: core test
