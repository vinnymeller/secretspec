{
  languages.rust.enable = true;
  languages.javascript.enable = true;
  languages.javascript.npm.enable = true;
  languages.javascript.npm.install.enable = true;

  git-hooks.hooks = {
    cargo-check.enable = true;
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
