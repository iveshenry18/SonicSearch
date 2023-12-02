# SonicSearch
A search engine for your sounds.

## Contributing

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


### Building
This app isn't exactly... portable yet. For now you need at least a few binaries. Here's how to install those on my system (MacOS with Apple Silicon).

1. onnxruntime: the runtime for [ONNX](https://onnxruntime.ai/), the system we use for inference. `brew install onnxruntime`
1. libomp: ??. `brew install libomp`
1. llvm: the build toolchain(?). `brew install llvm`

These installations correspond to the values in `env_vars.example.sh`. After installing these, copy that file to `env_vars.local.sh`. Either run that file manually, or it will attempt to run before calling tauri commands. Not a great system.

Faiss requires these env vars on Mac M1 per [this issue](https://github.com/facebookresearch/faiss/issues/2111)

#### Faiss
This is hell. I'm attempting to build Faiss from source on M1 Mac such that faiss-rs has something to dynamically link to. Basically, this means making sure libomp and llvm are installed using Homebrew, then running the standard cmake install steps (`cmake -B build .`, `cmake --build build`, `make -C build -j8 faiss_c`) prefixed with the proper env vars to point to the Homebrew libomp and llvm includes/libs. This is largely based on https://github.com/Enet4/faiss-rs/issues/74. Here's what I did:

1. Git clone the faiss-rs fork of faiss: `git clone https://github.com/Enet4/faiss.git && mv faiss enet4-faiss && cd enet4-faiss`
1. Cmake build stuff:
    1. `LDFLAGS="-L/opt/homebrew/opt/llvm/lib -L/opt/homebrew/opt/libomp/lib" CPPFLAGS="-I/opt/homebrew/opt/llvm/include -I/opt/homebrew/opt/libomp/include" CXX=/opt/homebrew/opt/llvm/bin/clang++ CC=/opt/homebrew/opt/llvm/bin/clang cmake -DFAISS_ENABLE_GPU=OFF -DFAISS_ENABLE_C_API=ON -DBUILD_SHARED_LIBS=ON -DCMAKE_BUILD_TYPE=Release -DFAISS_ENABLE_PYTHON=OFF -B build .`
    1. `LDFLAGS="-L/opt/homebrew/opt/llvm/lib -L/opt/homebrew/opt/libomp/lib" CPPFLAGS="-I/opt/homebrew/opt/llvm/include -I/opt/homebrew/opt/libomp/include" CXX=/opt/homebrew/opt/llvm/bin/clang++ CC=/opt/homebrew/opt/llvm/bin/clang cmake --build build`
    1. `LDFLAGS="-L/opt/homebrew/opt/llvm/lib -L/opt/homebrew/opt/libomp/lib" CPPFLAGS="-I/opt/homebrew/opt/llvm/include -I/opt/homebrew/opt/libomp/include" CXX=/opt/homebrew/opt/llvm/bin/clang++ CC=/opt/homebrew/opt/llvm/bin/clang make -C build -j8 faiss_c`
5. Move the dylibs to?? the Tauri binaries directory `cp build/c_api/libfaiss_c.dylib ../SonicSearch/sonicsearch-app/src-tauri/binaries` or maybe your local lib: 
    1. `sudo cp build/c_api/libfaiss_c.dylib /usr/local/lib/`

You'll also need to create the appropriate onnx models to populate the `SonicSearch/src-tauri/onnx_models` directory. That should be achievable simply by running all cells of `clap_export/clap_export.ipynb`.

Oh you'll also need to set up the [sqlx cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query)