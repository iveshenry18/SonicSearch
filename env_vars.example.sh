#! /bin/bash
# Full disclosure this may no longer be necessary

export ORT_DYLIB_PATH='/opt/homebrew/Cellar/onnxruntime/1.16.1/lib/libonnxruntime.dylib'
export LDFLAGS='-L/opt/homebrew/opt/llvm/lib'
export CPPFLAGS='-I/opt/homebrew/opt/llvm/include'
export CXX='/opt/homebrew/opt/llvm/bin/clang++'
export CC='/opt/homebrew/opt/llvm/bin/clang'
export RUSTFLAGS='-L/opt/homebrew/opt/libomp/lib -L/opt/homebrew/opt/llvm/lib -lblas -llapack'