# Simple gRPC async streaming server example

## How to build server

```
conan install -if conan_install .
mkdir build
cd build
cmake -DCMAKE_PROJECT_INCLUDE=../conan_install/conan_paths.cmake .
cmake --build .
```
