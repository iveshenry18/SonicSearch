# SonicSearch
A search engine for your sounds.

## Contributing

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


### Building
This app isn't exactly... portable yet. For now you need at least a few binaries. Here's how to install those on my system (MacOS with Apple Silicon).

1. onnxruntime: `brew install onnxruntime`
1. libomp: `brew install libomp`
1. llvm: `brew install llvm`

These installations correspond to the values in `env_vars.example.sh`. After installing these, copy that file to `env_vars.local.sh`. Either run that file manually, or it will attempt to run before calling tauri commands. Not a great system.

Faiss requires these env vars on Mac M1 per [this issue](https://github.com/facebookresearch/faiss/issues/2111)

You'll also need to create the appropriate onnx models to populate the `SonicSearch/src-tauri/onnx_models` directory. That should be achievable simply by running all cells of `clap_export/clap_export.ipynb`.

Oh you'll also need to set up the [sqlx cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query)