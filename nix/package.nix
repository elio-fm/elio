{
  lib,
  stdenv,
  rustPlatform,
  pkg-config,
  zstd,
  bash,
  zsh,
  fish,
  nushell,
  coreutils,
  procps,
  _7zz,
  ffmpeg,
}:
let
  manifest = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package;
in
rustPlatform.buildRustPackage {
  pname = "elio";
  inherit (manifest) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../build.rs
      ../src
      ../tests
      ../assets
      ../examples
      ../packaging/linux
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ pkg-config ];
  # Darwin stdenv supplies the Apple SDK, including Foundation, CoreFoundation,
  # CoreServices and CoreText; separate legacy framework inputs are unnecessary.
  buildInputs = [ zstd ];
  env.ZSTD_SYS_USE_PKG_CONFIG = "1";

  # Exercise the shell runtime tests instead of silently skipping absent shells.
  nativeCheckInputs = [
    bash
    zsh
    fish
    nushell
    coreutils # cat and sleep for isolated clipboard fixtures
    _7zz
    ffmpeg
  ] ++ lib.optionals stdenv.hostPlatform.isLinux [ procps ];

  # Tests mutate process-wide environment variables across several modules.
  doCheck = true;
  checkFlags = [ "--test-threads=1" ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
    # These integration tests need Finder and a user session unavailable to the
    # Nix builder. They remain enabled in ordinary macOS CI.
    "--skip=file_operations::tests::trash_delete::confirm_trash_batch_single_file_shows_quoted_name"
    "--skip=file_operations::tests::trash_delete::confirm_trash_batch_trashes_multiple_files_and_reports_count"
    # Parent-shell detection needs ps, which is unavailable/restricted in the
    # Darwin builder. Ordinary macOS CI still exercises real process inspection.
    "--skip=shell_install_detects_current_parent_shell_before_login_shell_environment"
    "--skip=shell_install_rejects_unsupported_current_shell_before_login_shell_environment"
    "--skip=shell_uninstall_rejects_unsupported_current_shell_before_login_shell_environment"
  ];

  postInstall = lib.optionalString stdenv.hostPlatform.isLinux ''
    install -Dm644 packaging/linux/elio.desktop "$out/share/applications/elio.desktop"
    mkdir -p "$out/share/icons"
    cp -r packaging/linux/icons/hicolor "$out/share/icons/"
  '';

  meta = {
    inherit (manifest) description;
    homepage = "https://elio-fm.github.io/";
    license = lib.licenses.mit;
    mainProgram = "elio";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
  };
}
