# Developer Setup

Install pre-commit via python to enforce pre commit hooks.
The hooks are defined in [.pre-commit-config.yaml](./.pre-commit-config.yaml)

```python
pip install pre-commit # installs pre-commit, may need to use pip3 depending on OS
pre-commit install # installs the pre-commit hooks in this repo to your .git folder
```

Now after you add your change to git, when you go to `git commit` the pre commit hooks will be ran.
This will enforce both clippy and cargo formatting are correct.
To fix clippy errors you will need to change the code manually based on the suggestions.
To fix formatting errors those can be fixed with `cargo +nightly fmt`.
If you have not installed the nightly toolchain, do so by `rustup install nightly`.
