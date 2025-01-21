# Installation

Kirin is available in [PyPI](https://pypi.org/) and
thus can be installed via [`pip`](https://pypi.org/project/pip/).
Install Kirin using the following command:

```bash
pip install kirin-toolchain
```

Kirin supports Python 3.9 or later. We recommend using Python 3.10+ for the best experience.

We strongly recommend developing your compiler project using [`uv`](https://docs.astral.sh/uv/),
which is the official development environment for Kirin. You can install `uv` using the following command:


=== "Linux and macOS"

    ```bash
    curl -LsSf https://astral.sh/uv/install.sh | sh
    ```

    then

    ```bash
    uv add kirin-toolchain
    ```

=== "Windows"

    ```cmd
    powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
    ```

    then

    ```cmd
    uv add kirin-toolchain
    ```

## Kirin and its friends

Kirin also comes with a few friends that you might find useful:

- `bloqade-qasm`: (link missing) A quantum assembly language (QASM 2.0) dialect for Kirin with a builtin QASM 2.0 text format parser.
- `bloqade`: (available soon) QuEra's SDK for next-gen error-corrected neutral-atom quantum computers.

## Development

If you want to contribute to Kirin, you can clone the repository from GitHub:

```bash
git clone https://github.com/QuEraComputing/kirin.git
```

We use `uv` to manage the development environment, after you install `uv`, you can install the development dependencies using the following command:

```bash
uv sync
```

Our code review requires that you pass the tests and the linting checks. We recommend
you to install `pre-commit` to run the checks before you commit your changes, the command line
tool `pre-commit` has been installed as part of the development dependencies. You can setup
`pre-commit` using the following command:

```bash
pre-commit install
```

## Requirements

Kirin requires the following dependencies:

- `rich`: for pretty-printing
- `type-extensions`: for type hints
- `beartype`: for runtime type checking and analysis of type hints in the python dialect
