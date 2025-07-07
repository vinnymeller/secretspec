{
  languages.rust.enable = true;

  pre-commit.hooks = {
    cargo-check.enable = true;
    rustfmt.enable = true;
    clippy.enable = true;
  };

  enterTest = ''
    cargo test --all --verbose
  '';
}
