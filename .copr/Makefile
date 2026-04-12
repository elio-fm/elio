SHELL := /usr/bin/bash
.SHELLFLAGS := -eo pipefail -c

REPO_ROOT := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST))))/..)

.PHONY: srpm
srpm:
	set -u; \
	$(MAKE) -C "$(REPO_ROOT)/packaging/fedora" srpm outdir="$(outdir)" spec="$(spec)" release="$(release)"
