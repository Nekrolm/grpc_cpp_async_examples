
add_executable(hello_server
    async_hello_server.cpp
)

target_link_libraries(hello_server
    PUBLIC
        hello_stream_proto
    PRIVATE
        fmt::fmt
)

add_executable(hello_server_with_rust
    async_hello_server_with_rust.cpp
)

target_link_libraries(hello_server_with_rust
    PUBLIC
        hello_stream_proto
    PRIVATE
        fmt::fmt
        generated_rust_bindings
)

