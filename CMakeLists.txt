cmake_minimum_required(VERSION 3.20)

set(CMAKE_CXX_STANDARD 20)

project(grpc_async_streaming)

find_package(gRPC REQUIRED EXACT 1.38)
find_package(Protobuf REQUIRED EXACT 3.17.1)
find_package(fmt REQUIRED EXACT 8.0.0)

add_subdirectory(protos)
add_subdirectory(src)
