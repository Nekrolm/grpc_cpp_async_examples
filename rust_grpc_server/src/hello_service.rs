use futures::Stream;

use tonic::Status;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{atomic, Arc, Mutex};

use tokio::sync::{mpsc, watch};
use tokio_stream;

pub mod bidi_hello {
    tonic::include_proto!("bidi_hello");
}

use bidi_hello::bidi_hello_service_server::BidiHelloService;

type HelloServiceOutputStream =
    Pin<Box<dyn Stream<Item = HelloResponseResult> + Send + Sync + 'static>>;

type HelloServiceInputStream = tonic::Streaming<bidi_hello::HelloRequest>;

struct HelloConnection {
    has_connection: Arc<atomic::AtomicBool>,

    response_queue: watch::Receiver<bidi_hello::HelloResponse>,
    request_queue: mpsc::Sender<bidi_hello::HelloRequest>,
}

impl HelloConnection {
    pub fn new(
        request_queue: mpsc::Sender<bidi_hello::HelloRequest>,
        response_queue: watch::Receiver<bidi_hello::HelloResponse>,
    ) -> Self {
        Self {
            has_connection: Arc::new(atomic::AtomicBool::new(false)),
            response_queue,
            request_queue,
        }
    }

    async fn run_communication(
        &self,
        mut input_stream: HelloServiceInputStream,
    ) -> Result<HelloServiceOutputStream, tonic::Status> {
        let has_connection = self.has_connection.clone();

        has_connection
            .compare_exchange(
                false,
                true,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            )
            .map_err(|_| Status::resource_exhausted("only one active connection supported"))?;

        let (tx, rx) = mpsc::channel(2);
        let mut response_queue = self.response_queue.clone();
        let tx_has_connection = has_connection.clone();
        tokio::spawn(async move {
            while response_queue.changed().await.is_ok() {
                if !tx_has_connection.load(atomic::Ordering::SeqCst) {
                    break;
                }
                let next_response = response_queue.borrow().clone();
                let send_result = tx.send(Ok(next_response)).await;
                if send_result.is_err() {
                    break;
                }
            }
            if tx.send(Err(Status::ok("OK"))).await.is_err() {
                println!("Something wring with final send");
            }
        });

        let request_queue = self.request_queue.clone();
        tokio::spawn(async move {
            while let Ok(Some(req)) = input_stream.message().await {
                if request_queue.send(req).await.is_err() {
                    break;
                }
            }
            has_connection.store(false, atomic::Ordering::SeqCst);
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}

pub struct BidiHelloServiceImpl {
    available_services: Mutex<HashMap<String, Arc<HelloConnection>>>,
}

impl BidiHelloServiceImpl {
    async fn accept_init_communication(
        &self,
        first: bidi_hello::HelloRequest,
        input_stream: HelloServiceInputStream,
    ) -> Result<HelloServiceOutputStream, tonic::Status> {
        let first_message = first
            .request
            .ok_or(tonic::Status::invalid_argument("empty first message"))?;

        match first_message {
            bidi_hello::hello_request::Request::Data(_) => {
                Err(tonic::Status::invalid_argument("first message has to be Selector"))
            }
            bidi_hello::hello_request::Request::Selector(selector) => {
                self.select_and_start_communication(selector, input_stream)
                    .await
            }
        }
    }

    async fn select_and_start_communication(
        &self,
        selector: bidi_hello::Selector,
        input_stream: HelloServiceInputStream,
    ) -> Result<HelloServiceOutputStream, tonic::Status> {
        let service = {
            let service_list = self
                .available_services
                .lock()
                .map_err(|_| tonic::Status::internal("internal mutex error"))?;
            service_list
                .get(&selector.service_name)
                .ok_or(tonic::Status::not_found("service name not found"))?
                .clone()
        };
        service.run_communication(input_stream).await
    }
}

type HelloResponseResult = Result<bidi_hello::HelloResponse, Status>;

type AvailableServicesResponse = tonic::Response<bidi_hello::AvailableServices>;

#[tonic::async_trait]
impl BidiHelloService for BidiHelloServiceImpl {
    type CommunicateStream = HelloServiceOutputStream;

    async fn list_available_services(
        &self,
        _: tonic::Request<bidi_hello::Empty>,
    ) -> Result<AvailableServicesResponse, tonic::Status> {
        let service_list = self
            .available_services
            .lock()
            .map_err(|_| tonic::Status::internal("internal mutex error"))?;
        Ok(tonic::Response::new(bidi_hello::AvailableServices {
            services: service_list.keys().map(|x| x.clone()).collect(),
        }))
    }

    async fn communicate(
        &self,
        request: tonic::Request<tonic::Streaming<bidi_hello::HelloRequest>>,
    ) -> Result<tonic::Response<Self::CommunicateStream>, tonic::Status> {
        let mut input_stream = request.into_inner();
        let first_message = input_stream.message().await?;

        let service_communicator = match first_message {
            Some(msg) => self.accept_init_communication(msg, input_stream).await,
            None => Err(Status::cancelled("cancelled by peer")),
        }?;

        Ok(tonic::Response::new(service_communicator))
    }
}

pub struct ServiceIO {
    pub response_tx: watch::Sender<bidi_hello::HelloResponse>,
    pub request_rx: mpsc::Receiver<bidi_hello::HelloRequest>,
}

impl BidiHelloServiceImpl {
    pub fn new() -> Self {
        Self {
            available_services: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_service(
        &mut self,
        service_name: String,
    ) -> Result<ServiceIO, Box<dyn std::error::Error>> {
        let mut services = self.available_services.lock().unwrap();

        let (response_tx, response_rx) = watch::channel(bidi_hello::HelloResponse {
            ..Default::default()
        });

        let (request_tx, request_rx) = mpsc::channel(10);

        let new_connection = Arc::new(HelloConnection::new(request_tx, response_rx));

        use std::collections::hash_map::Entry;
        if let Entry::Vacant(o) = services.entry(service_name) {
            o.insert(new_connection);
        } else {
            return Err(format!("Can't register service. Already exists").into());
        }

        Ok(ServiceIO {
            response_tx,
            request_rx,
        })
    }
}

pub struct ServerRuntimeBuilder {
    rt: tokio::runtime::Runtime,
    service: BidiHelloServiceImpl,
}

pub struct RunningServer(tokio::runtime::Runtime);

pub type HelloResponseHandler = watch::Sender<bidi_hello::HelloResponse>;

impl ServerRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            rt: tokio::runtime::Runtime::new().unwrap(),
            service: BidiHelloServiceImpl::new(),
        }
    }

    pub fn register_service<F>(
        &mut self,
        name: String,
        on_recv: F,
    ) -> Result<HelloResponseHandler, Box<dyn std::error::Error>>
    where
        F: Fn(bidi_hello::HelloRequest) -> () + Send + 'static,
    {
        let ServiceIO {
            mut request_rx,
            response_tx,
        } = self.service.register_service(name)?;

        self.rt.spawn(async move {
            while let Some(req) = request_rx.recv().await {
                on_recv(req);
            }
        });

        Ok(response_tx)
    }

    pub fn run_server(self, port: u16) -> RunningServer {
        let server =
            bidi_hello::bidi_hello_service_server::BidiHelloServiceServer::new(self.service);
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        self.rt.spawn(async move {
            tonic::transport::Server::builder()
                .add_service(server)
                .serve(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port))
                .await
                .unwrap();
        });

        RunningServer(self.rt)
    }
}
