#! /bin/bash
# This sets the necessary environment variables for building the application. It might only apply to my system.

echo "Setting environment variables"

export ORT_DYLIB_PATH='/opt/homebrew/Cellar/onnxruntime/1.16.1/lib/libonnxruntime.dylib'
export LDFLAGS='-L/opt/homebrew/opt/llvm/lib -L/opt/homebrew/opt/libomp/lib'
export LIBRARY_PATH="/opt/homebrew/opt/llvm/lib:/opt/homebrew/opt/libomp/lib"
export CPPFLAGS='-I/opt/homebrew/opt/llvm/include -I/opt/homebrew/opt/libomp/include'
export CXX='/opt/homebrew/opt/llvm/bin/clang++'
export CC='/opt/homebrew/opt/llvm/bin/clang'
export RUSTFLAGS='-L/opt/homebrew/opt/libomp/lib -L/opt/homebrew/opt/llvm/lib -lblas -llapack'

echo "Environment variables set"