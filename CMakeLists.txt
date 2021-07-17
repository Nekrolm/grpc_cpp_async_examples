cmake_minimum_required(VERSION 3.20)

set(CMAKE_CXX_STANDARD 20)

project(grpc_async_streaming)

find_package(gRPC REQUIRED EXACT 1.38)
find_package(Protobuf REQUIRED EXACT 3.17.1)
find_package(fmt REQUIRED EXACT 8.0.0)


include(FetchContent)

FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/AndrewGaspar/corrosion.git
    GIT_TAG origin/master # Optionally specify a version tag or branch here
)

FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(MANIFEST_PATH rust_grpc_server/Cargo.toml)

add_custom_target(generated_binginds
    COMMAND cbindgen --config ${CMAKE_CURRENT_SOURCE_DIR}/rust_grpc_server/cbindgen.toml 
                     --output ${CMAKE_CURRENT_BINARY_DIR}/rust_grpc_server.hpp
                     --lang C++
                     ${CMAKE_CURRENT_SOURCE_DIR}/rust_grpc_server)
add_library(generated_rust_bindings INTERFACE)
target_include_directories(generated_rust_bindings INTERFACE 
            ${CMAKE_CURRENT_BINARY_DIR})
target_link_libraries(generated_rust_bindings
            INTERFACE rust_grpc_server)   
add_dependencies(generated_rust_bindings generated_binginds)

add_subdirectory(protos)
add_subdirectory(src)
