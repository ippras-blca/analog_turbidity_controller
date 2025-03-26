use crate::turbidity::Request as TurbidityRequest;
use anyhow::Result;
use log::{error, info};
use std::{net::SocketAddr, sync::LazyLock};
use tokio::{
    net::TcpListener,
    sync::{mpsc::Sender, oneshot},
};
use tokio_modbus::{
    prelude::*,
    server::{
        Service,
        tcp::{Server, accept_tcp_connection},
    },
};

static SOCKET_ADDR: LazyLock<SocketAddr> = LazyLock::new(|| "0.0.0.0:5502".parse().unwrap());

pub(super) async fn run(turbidity_sender: Sender<TurbidityRequest>) -> Result<()> {
    let server = Server::new(TcpListener::bind(*SOCKET_ADDR).await?);
    let new_service = |_socket_addr| Ok(Some(ExampleService::new(turbidity_sender.clone())));
    let on_connected = |stream, socket_addr| async move {
        accept_tcp_connection(stream, socket_addr, new_service)
    };
    let on_process_error = |error| error!("{error}");
    server.serve(&on_connected, on_process_error).await?;
    Ok(())
}

struct ExampleService {
    turbidity_sender: Sender<TurbidityRequest>,
}

impl ExampleService {
    fn new(turbidity_sender: Sender<TurbidityRequest>) -> Self {
        Self { turbidity_sender }
    }
}

impl Service for ExampleService {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = impl Future<Output = Result<Self::Response, Self::Exception>>;

    fn call(&self, request: Self::Request) -> Self::Future {
        info!("Modbus request: {request:?}");
        let turbidity_sender = self.turbidity_sender.clone();
        async move {
            match request {
                Request::ReadInputRegisters(address, count) => {
                    if address != 0 || count != 1 {
                        error!("IllegalAddress {{ address: {address}, count: {count} }}");
                        return Err(ExceptionCode::IllegalDataAddress);
                    }
                    let (sender, receiver) = oneshot::channel();
                    if let Err(error) = turbidity_sender.send(sender).await {
                        error!("{error:?}");
                        return Err(ExceptionCode::ServerDeviceFailure);
                    };
                    let input_registers = match receiver.await {
                        Ok(Ok(turbidity)) => vec![turbidity],
                        Ok(Err(error)) => {
                            error!("{error:?}");
                            return Err(ExceptionCode::ServerDeviceFailure);
                        }
                        Err(error) => {
                            error!("{error:?}");
                            return Err(ExceptionCode::ServerDeviceFailure);
                        }
                    };
                    Ok(Response::ReadInputRegisters(input_registers))
                }
                _ => Err(ExceptionCode::IllegalFunction),
            }
        }
    }
}
