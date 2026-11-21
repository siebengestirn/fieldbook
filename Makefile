# Repository task entry point.
#
# The Makefile is a thin, deterministic façade over `cargo xtask`, which owns
# the authoritative gate definitions. Style rules follow the Linux kernel
# coding guidelines and the Google Python, Shell, Markdown, JSON and Rust
# style guides; see docs/style-guide.md.
#
# Recipe lines use tabs (required by make). Every target is phony: there is no
# file produced by any target name, so `.PHONY` prevents stale-file skips.

.PHONY: all check ci style style-fix fmt lint test docs e2e verify \
	python-check ruff-check q-check standards-check python-fmt shell-check \
	json-check markdown-check makefile-check rust-check tools

.DEFAULT_GOAL := check

SHELL := /bin/bash

all: check

check: style ci

ci:
	cargo xtask ci

fmt: style-fix

style: python-check ruff-check q-check standards-check shell-check \
	json-check markdown-check makefile-check

style-fix:
	python3 scripts/check_style.py --fix

python-check:
	python3 scripts/check_style.py --python

ruff-check:
	python3 scripts/ruff_gate.py .

q-check:
	python3 scripts/check_q_style.py
	python3 scripts/check_q_style.py --self-test

standards-check:
	python3 scripts/check_prose_unified.py --self-test
	python3 scripts/check_prose_unified.py
	python3 scripts/check_quality_contract.py --self-test
	python3 scripts/check_quality_contract.py
	python3 scripts/check_license_provenance.py --self-test
	python3 scripts/check_license_provenance.py

python-fmt:
	python3 -m pyink --config pyproject.toml scripts

shell-check:
	python3 scripts/check_style.py --shell
	python3 scripts/check_shellcheck.py

json-check:
	python3 scripts/check_style.py --json

markdown-check:
	python3 scripts/check_style.py --markdown

makefile-check:
	python3 scripts/check_style.py --makefile

rust-check:
	cargo xtask fmt
	cargo xtask lint

lint: rust-check python-check ruff-check q-check standards-check shell-check

test:
	cargo xtask test

docs:
	cargo xtask docs

e2e:
	cargo xtask e2e

verify:
	cargo xtask verify

tools:
	python3 -m pip install -r requirements-dev.txt
