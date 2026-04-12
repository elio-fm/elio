# Fedora COPR packaging

This directory contains the Fedora RPM packaging for COPR.

COPR's `make_srpm` SCM build method expects a repository-root `.copr/Makefile`
with an `srpm` target. The root `.copr/Makefile` in this repo is intentionally
only a shim; the real packaging logic stays here.

## Local SRPM test

```bash
make -f .copr/Makefile srpm outdir=/tmp/elio-copr-srpm spec=packaging/fedora/elio.spec release=1
```

## Manual COPR test build

Build an SRPM locally from the current checkout, then submit that SRPM to COPR:

```bash
make -f .copr/Makefile srpm outdir=/tmp/elio-copr-srpm spec=packaging/fedora/elio.spec release=1
copr-cli build elio /tmp/elio-copr-srpm/elio-*.src.rpm
```

Use a higher `release=` value for same-version packaging-only rebuilds that
should upgrade over an existing RPM.

## Release automation

The GitHub release workflow builds the SRPM from the release checkout and
submits it to COPR with RPM release `1`. For same-version Fedora packaging-only
rebuilds, build and submit an SRPM manually with a higher `release=` value.

Configure the `COPR_CONFIG` secret in GitHub with the full contents of
`~/.config/copr`.

The COPR API token expires; rotate the secret before the expiration date shown
on the COPR API page.
