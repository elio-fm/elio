# Fedora COPR packaging

This directory contains the Fedora RPM packaging for COPR.

COPR's `make_srpm` SCM build method expects a repository-root `.copr/Makefile`
with an `srpm` target. The root `.copr/Makefile` in this repo is intentionally
only a shim; the real packaging logic stays here.

## Local SRPM test

```bash
make -f .copr/Makefile srpm outdir=/tmp/elio-copr-srpm spec=packaging/fedora/elio.spec
```

## Manual COPR test build

After committing and pushing this packaging to GitHub, trigger the first build
from the branch that contains it:

```bash
copr-cli buildscm elio \
  --clone-url https://github.com/MiguelRegueiro/elio.git \
  --commit main \
  --spec packaging/fedora/elio.spec \
  --method make_srpm
```

Do not pass `--subdir packaging/fedora` with this layout. COPR should run the
root `.copr/Makefile`, which delegates back into this directory.

## Release automation

The GitHub release workflow triggers COPR from the release tag. Configure the
`COPR_CONFIG` secret in GitHub with the full contents of `~/.config/copr`.

The COPR API token expires; rotate the secret before the expiration date shown
on the COPR API page.
