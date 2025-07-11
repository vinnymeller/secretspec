{ pkgs, ... }: {
  languages.rust.enable = true;
  languages.javascript = {
    enable = true;
    npm = {
      enable = true;
      install.enable = true;
    };
  };

  packages = [
    # keyring
    pkgs.dbus
    # coverage testing
    pkgs.cargo-tarpaulin
  ];

  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };

  enterTest = ''
    cargo test --all --verbose
  '';

  processes.docs.exec = ''
    cd docs && astro dev
  '';
}
