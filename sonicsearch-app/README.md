# SonicSearch
A search engine for your sounds.

## Contributing

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


### Building
This app isn't exactly... portable yet. For now you need at least a few binaries. At present, the `package.json` includes env variables that assume you're running MacOS and have these installed in the same place I do... :)

1. onnxruntime: `brew install onnxruntime`
1. libomp: `brew install libomp`
1. llvm: `brew install llvm`

You'll also need to create the appropriate onnx models to populate the `SonicSearch/src-tauri/onnx_models` directory. That should be achievable simply by running all cells of `clap_export/clap_export.ipynb`.

Oh you'll also need to set up the [sqlx cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query)