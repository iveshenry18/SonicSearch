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

## Distribution

Follow the [Tauri MacOS Distribution Guide](https://tauri.app/v1/guides/distribution/sign-macos/). This includes creating a CSR (locally), using that to create a certificate (via Apple Developer), then setting the following

```
export APPLE_SIGNING_IDENTITY=<the result of `security find-identity -v -p codesigning`>

export APPLE_ID=<swag@icloud.gov>
export APPLE_PASSWORD=<app-specific password>
export APPLE_TEAM_ID=<team ID, found in Apple Developer dashboard>
```

Once these are properly configured, you should be able to build a signed & notarized version of the app with `bun tauri build`

### Gotchas

The certificate biz was hellish. I got "Warning: unable to build chain to self-signed root for signer". This helped a little: https://stackoverflow.com/questions/48911289/warning-unable-to-build-chain-to-self-signed-root-for-signer-warning-in-xcode

- Remember that certificates are a chain of trusted parent-child relationships all the way up to a Root certificate. You can right-click on your Developer ID Application cert and hit Evaluate. If it's configured correctly, you should see a list that includes a Root cert (likely Apple Root CA). In my case, the intermediate step is the Developer ID Certificate Authority, which I downloaded from [here](https://www.apple.com/certificateauthority/).

The `xcrun notarytool submit ...` command "typically takes less than an hour" per the [docs](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow/). The first time it took a whole weekend (!), and since then it's been taking ~5–10 minutes.
TL;DR: Expect the `bun tauri build` script to hang on this step for a while.
