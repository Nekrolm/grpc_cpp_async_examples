
add_executable(hello_server
    async_hello_server.cpp
)

target_link_libraries(hello_server
    PUBLIC
        hello_stream_proto
    PRIVATE
        fmt::fmt
)
