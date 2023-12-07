# SonicSearch
A search engine for your sounds.

## Contributing

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


### Building
You'll need at least a few binaries. Here's how to install those on my system (MacOS with Apple Silicon).

1. onnxruntime: the runtime for [ONNX](https://onnxruntime.ai/), the system we use for inference. `brew install onnxruntime`
1. libomp: A library for.. parallel processing. `brew install libomp`
1. llvm: the build toolchain(?). `brew install llvm`

These installations correspond to the values in `env_vars.example.sh`. After installing these, copy that file to `env_vars.local.sh`. Either run that file manually, or it will attempt to run before calling tauri commands. Not a great system.

You'll also need to create the appropriate onnx models to populate the `SonicSearch/src-tauri/onnx_models` directory. That should be achievable simply by running all cells of `clap_export/clap_export.ipynb`.

Oh you'll also need to set up the [sqlx cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query)

## Bundling
Real actual bundling requires the appropriate .dylibs on MacOS. You'll need to create a `libs` directory under `src-tauri` and copy libomp.dylib and libonnxruntime.dylib to it. The commands will be something like
```
cp /opt/homebrew/Cellar/onnxruntime/1.16.1/lib/libonnxruntime.dylib ./src-tauri/libs
```
```
cp /opt/homebrew/Cellar/llvm/17.0.6/lib/libomp.dylib ./src-tauri/libs/
```