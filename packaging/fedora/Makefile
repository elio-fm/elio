SHELL := /usr/bin/bash
.SHELLFLAGS := -eo pipefail -c

NAME := elio
FEDORA_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
REPO_ROOT := $(abspath $(FEDORA_DIR)/../..)

.PHONY: srpm
srpm:
	set -u; \
	mkdir -p "$(outdir)"; \
	outdir="$$(realpath "$(outdir)")"; \
	version="$$(sed -n 's/^version = "\(.*\)"/\1/p' "$(REPO_ROOT)/Cargo.toml" | head -n1)"; \
	release="$(release)"; \
	release="$${release:-1}"; \
	if [[ ! "$$release" =~ ^[1-9][0-9]*$$ ]]; then \
		echo "release must be a positive integer, got '$$release'" >&2; \
		exit 1; \
	fi; \
	workdir="$$(mktemp -d)"; \
	trap 'rm -rf "$$workdir"' EXIT; \
	if [[ "$$(id -u)" -eq 0 ]] && command -v dnf >/dev/null 2>&1; then \
		dnf -y install cargo git rpm-build zstd; \
	fi; \
	mkdir -p "$$outdir" "$$workdir/build" "$$workdir/rpms"; \
	git -C "$(REPO_ROOT)" archive --format=tar --prefix="$(NAME)-$$version/" HEAD | tar -C "$$workdir" -xf -; \
	cp "$(FEDORA_DIR)/$(NAME).spec" "$$workdir/"; \
	tar -C "$$workdir" -czf "$$outdir/$(NAME)-$$version.tar.gz" "$(NAME)-$$version"; \
	cd "$(REPO_ROOT)" && cargo vendor --locked "$$workdir/vendor" >/dev/null; \
	tar -C "$$workdir" -caf "$$outdir/vendor-$$version.tar.zst" vendor; \
	rpmbuild -bs "$$workdir/$(NAME).spec" \
		--define "elio_version $$version" \
		--define "elio_release $$release" \
		--define "_sourcedir $$outdir" \
		--define "_srcrpmdir $$outdir" \
		--define "_builddir $$workdir/build" \
		--define "_rpmdir $$workdir/rpms" \
		--define "_specdir $$workdir"
