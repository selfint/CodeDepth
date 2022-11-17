# CodeDepth
Analyze the depth of each method using the language's LSP server

NOTE: this project requires the LSP server to already be installed.

## Usage

```shell
$ code_depth -p path/to/project/root -l "cmd to run to start lsp server"
```

## Example - rust_analyzer

1. Install rust analyzer for your platform from the [newest release](https://github.com/rust-lang/rust-analyzer/releases/latest)
2. Run `code_depth` on your project with rust_analyzer:


```shell
$ code_depth -p path/to/project/root -l rust_analyzer
```
