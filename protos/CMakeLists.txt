

add_library(hello_stream_proto hello_stream.proto)
target_link_libraries(hello_stream_proto
    PUBLIC
        gRPC::grpc
        gRPC::grpc++
        protobuf::protobuf
        ${protobuf_LIBRARIES} # due to bug in FindProtobuf.cmake
)

target_include_directories(hello_stream_proto
    PUBLIC
        ${CMAKE_CURRENT_BINARY_DIR}
)

get_target_property(grpc_cpp_plugin_location gRPC::grpc_cpp_plugin LOCATION)
# compile the message types
protobuf_generate(TARGET hello_stream_proto LANGUAGE cpp)

# compile the GRPC services
protobuf_generate(
    TARGET
        hello_stream_proto
    LANGUAGE
        grpc
    GENERATE_EXTENSIONS
        .grpc.pb.h
        .grpc.pb.cc
    PLUGIN
        "protoc-gen-grpc=${grpc_cpp_plugin_location}"
)