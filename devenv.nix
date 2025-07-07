{
  languages.rust.enable = true;

  git-hooks.hooks = {
    cargo-check.enable = true;
    rustfmt.enable = true;
    clippy.enable = true;
  };

  enterTest = ''
    cargo test --all --verbose
  '';
}
