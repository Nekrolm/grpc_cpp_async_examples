#include <fmt/format.h>

#include <hello_stream.pb.h>
#include <hello_stream.grpc.pb.h>

#include <grpc/grpc.h>
#include <grpc++/grpc++.h>


#include <thread>
#include <memory>
#include <string>
#include <functional>
#include <mutex>
#include <condition_variable>

struct RPCEvent {
    // ok -- status returned from grpc::CompletionQueue
    // At most cases `false` means event was cancelled
    virtual void Proceed(bool grpc_event_ok = true) = 0;
    void* as_tag() {
        return this;
    }
protected:
    virtual ~RPCEvent() = default;
};


struct Connection {
    explicit Connection(grpc::CompletionQueue* cq,
                        std::unique_ptr<grpc::ServerContext>&& ctx, 
                        std::unique_ptr<hello::Empty>&& req,
                        std::unique_ptr<grpc::ServerAsyncWriter<hello::Text>>&& 
                            responder) : 
        cq_{cq},
        context_{std::move(ctx)},
        request_{std::move(req)},
        responder_{std::move(responder)}        
        {}

   
    using OnEventCompletion = std::function<void(void)>;


    void Write(const hello::Text& val, std::optional<OnEventCompletion> on_completion = std::nullopt) {
        // Async gRPC API allows writing only one message into stream at time.
        // responder == nullptr indicates that there is previous pending write/finish event 
        // so we have to wait up to event completion.


        // if (context_->IsCancelled()) {
        //  notify somehow ?
        // }
        std::unique_lock lock{ mutex_};
        cv_.wait(lock, [this] {
            return responder_ != nullptr;
        });
        if (!cq_) {
            return; // TODO: error ?
        }

        fmt::print("Try write {:x}\n", reinterpret_cast<uintptr_t>(this));
        event_->on_completion_ = std::move(on_completion);
        // transfer responder into pending event.
        // When event is completed it will return back.
        event_->responder_ = std::move(responder_);
        // enqueue event into grpc::CompletionQueue
        event_->responder_->Write(val, event_->as_tag());
    }

    void Finish(grpc::Status status, std::optional<OnEventCompletion> on_completion = std::nullopt) {
        std::unique_lock lock{ mutex_};
        cv_.wait(lock, [this] {
            return responder_ != nullptr;
        });
        if (!cq_) {
            return;
        }
        event_->on_completion_ = std::move(on_completion);
        event_->responder_ = std::move(responder_);
        event_->responder_->Finish(status, event_->as_tag());
        cq_ = nullptr;
    }

    bool Finished() const {
        std::unique_lock lock{ mutex_};
        return cq_ == nullptr;
    }

    ~Connection() {
        Finish(grpc::Status::CANCELLED);        
    }



private:
    const grpc::CompletionQueue* cq_;
    const std::unique_ptr<grpc::ServerContext> context_;
    const std::unique_ptr<hello::Empty> request_;

    // ----------------------
    std::unique_ptr<grpc::ServerAsyncWriter<hello::Text>> responder_;
    mutable std::mutex mutex_;
    mutable std::condition_variable cv_;
    // ---------------------

    struct WritingEvent : RPCEvent {

        explicit WritingEvent(Connection* parent) : parent { parent }{}

        Connection* parent;
        std::unique_ptr<grpc::ServerAsyncWriter<hello::Text>> responder_;
        std::optional<OnEventCompletion> on_completion_;

        void Proceed(bool grpc_event_ok) override {
            if (grpc_event_ok) {
                if (on_completion_) {
                    (*on_completion_)();
                }
            }
            std::lock_guard lock {parent->mutex_};
            parent->responder_ = std::move(responder_);
            parent->cv_.notify_one();
        }
    };


    std::unique_ptr<WritingEvent> event_ = std::make_unique<WritingEvent>(this);
};

class HelloServerImpl {
public:
    ~HelloServerImpl() {
        server_->Shutdown();
        cq_->Shutdown();
        queue_worker_.join();
    }

    explicit HelloServerImpl(uint16_t port) {
        const std::string server_address = fmt::format("0.0.0.0:{}",port);
        grpc::ServerBuilder builder;
        builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
        builder.RegisterService(&service_);
        cq_ = builder.AddCompletionQueue();
        server_ = builder.BuildAndStart();
        fmt::print("Server listening on {}\n", server_address);
        queue_worker_ = std::thread([this]{
            this->HandleRPCs();
        });
    }

    using OnConnection = std::function<void(std::unique_ptr<Connection>&&)>;

    void ListenConnections(OnConnection on_connection) {
        // Clients can connect to server only if it is ready to accept their requests
            
       if (new_connection_handler_) {
           return; // Already listening
       }

       new_connection_handler_ = std::make_unique<NewConnectionHandler> ( std::move(on_connection), this );
    }

private:
    void HandleRPCs() {
        fmt::print("Start RPC handling\n");
        void* event_tag = nullptr;
        bool ok = false;
        while (cq_->Next(&event_tag, &ok)) {
            fmt::print("Handle event: {:x}\n", reinterpret_cast<uintptr_t>(event_tag));
            static_cast<RPCEvent*>(event_tag)->Proceed(ok);
        }
    }
    hello::HelloService::AsyncService service_;
    std::unique_ptr<grpc::ServerCompletionQueue> cq_;
    std::unique_ptr<grpc::Server> server_;
    std::thread queue_worker_;
    

     struct NewConnectionHandler : RPCEvent {
            OnConnection on_connect_;
            HelloServerImpl* self_;

            explicit NewConnectionHandler(OnConnection on_connect, HelloServerImpl* self) : 
                on_connect_ {std::move(on_connect)},
                self_{self} {
                    PreparePlaceholders();
            }

            std::unique_ptr<grpc::ServerContext> ctx_;
            std::unique_ptr<hello::Empty> request_;
            std::unique_ptr<grpc::ServerAsyncWriter<hello::Text>> responder_;
            
            void PreparePlaceholders() {
                ctx_ = std::make_unique<grpc::ServerContext>();
                request_ = std::make_unique<hello::Empty>();
                responder_ = std::make_unique<grpc::ServerAsyncWriter<hello::Text>>(ctx_.get());

                self_->service_.RequestOutputStream(ctx_.get(),
                                     request_.get(),
                                     responder_.get(),
                                     self_->cq_.get(),
                                     self_->cq_.get(),
                                     as_tag());
            }

            void Proceed(bool ok) override {
                if (ok) {
                    // Await new clients one-by-one
                    // Accept current and wait for next
                    on_connect_(std::make_unique<Connection>(
                        self_->cq_.get(),
                        std::move(ctx_),
                        std::move(request_),
                        std::move(responder_)
                    ));
                    PreparePlaceholders();
                }
                
            }
        };
    
    std::unique_ptr<NewConnectionHandler> new_connection_handler_;
};


int main() {
    HelloServerImpl impl { 8081 };

    std::mutex connections_mutex;
    std::vector<std::unique_ptr<Connection>> connections;

    const std::function<void(std::unique_ptr<Connection>&&)> on_connection = [&] (std::unique_ptr<Connection>&& conn) {
            std::lock_guard lock { connections_mutex };
            connections.push_back(std::move(conn));
    };

    impl.ListenConnections(std::cref(on_connection));

    std::vector<Connection*> connections_to_process;
    while (true) {
        usleep(500'000);
        fmt::print("NEXT SEND ITERATION\n");
        {
            std::lock_guard lock { connections_mutex };
            connections.erase(
                std::remove_if(connections.begin(),
                 connections.end(),
                 [](auto&& x) {
                     return x->Finished();
                 }),
                connections.end()
            );
            connections_to_process.clear();

            std::transform(
                connections.begin(),
                connections.end(),
                std::back_inserter(connections_to_process),
                [](const auto& x) { return x.get(); }
            );
        }

        for (int i = 0; i < 5; ++i) {
            hello::Text txt;
            txt.set_text(fmt::format("Text!!! {}", i));
            for (auto c : connections_to_process) {
                c->Write(txt); // async write!
            }
        }
        for (auto c : connections_to_process) {
            if (rand() % 2) {
                c->Finish(grpc::Status::OK);
            }
        }
    }
}