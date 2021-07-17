#include <fmt/format.h>

#include <iostream>

#include <unistd.h>

#include <rust_grpc_server.hpp>


int main() {

    std::cout << "Create builder!\n";
    auto builder_ctx = create_hello_server_builder();

    std::cout << "Buider created!\n";

    std::cout << "Register services: \n";
    auto tx1  = register_hello_service(builder_ctx, "hello1", 
    +[](const char* msg, void*){
        std::cout << fmt::format("Hello1: recv={}\n", msg);
    }, nullptr);

    auto tx2  = register_hello_service(builder_ctx, "hello2", 
    +[](const char* msg, void*){
        std::cout << fmt::format("Hello2: recv={}\n", msg);
    }, nullptr);

    std::cout << "Starting server...\n";
    auto runing = start_hello_server(builder_ctx, 8080);

    std::cout << "Go communicate!\n";
    for (int i = 0; i < 100; ++i) {
        usleep(500'000);
        std::cout << "iter " << i << "\n";
        const std::string msg = fmt::format("message {}", i);
        send_hello_service(tx1, msg.c_str());
        send_hello_service(tx2, msg.c_str());
    }

    std::cout << "stop communicate!\n";

    dispose_hello_service(tx1);
    dispose_hello_service(tx2);
    terminate_hello_server(runing);

    std::cout << "goodbye!\n";
}