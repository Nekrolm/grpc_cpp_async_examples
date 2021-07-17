use hello_service::ServerRuntimeBuilder;
pub use libc::{c_char, c_void};
use std::ffi::CStr;

mod hello_service;

pub type RecvHandler = unsafe extern "C" fn(*const c_char, *mut c_void) -> ();

pub struct ResponseHandler(hello_service::HelloResponseHandler);

struct UnsafeContext {
    ctx: *mut c_void,
}

unsafe impl Send for UnsafeContext {}
unsafe impl Sync for UnsafeContext {}

#[no_mangle]
pub extern "C" fn create_hello_server_builder() -> *mut ServerRuntimeBuilder {
    let rt_builder = Box::new(hello_service::ServerRuntimeBuilder::new());
    Box::into_raw(rt_builder)
}

#[no_mangle]
pub unsafe extern "C" fn register_hello_service(
    builder: *mut ServerRuntimeBuilder,
    name: *const c_char,
    on_recv: RecvHandler,
    recv_ctx: *mut c_void,
) -> *mut ResponseHandler {
    let ctx = UnsafeContext { ctx: recv_ctx };

    let tx = builder
        .as_mut()
        .zip(CStr::from_ptr(name).to_str().ok().map(|x| x.to_string()))
        .and_then(|(blder, name)| {
            blder
                .register_service(name, move |request| {
                    if let Some(req) = request.request {
                        match req {
                            hello_service::bidi_hello::hello_request::Request::Data(d) => {
                                let s = std::ffi::CString::new(d.text).expect("");
                                unsafe { on_recv(s.as_ptr(), ctx.ctx) }
                            }
                            _ => (),
                        }
                    }
                })
                .ok()
        });

    if let Some(tx) = tx {
        return Box::into_raw(Box::new(ResponseHandler(tx)));
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn send_hello_service(tx: *mut ResponseHandler, msg: *const c_char) {
    let tx_result = tx.as_mut().zip(CStr::from_ptr(msg).to_str().ok()).and_then(
        |(ResponseHandler(tx), msg)| {
            tx.send(hello_service::bidi_hello::HelloResponse {
                text: msg.to_owned(),
            })
            .ok()
        },
    );
    if tx_result.is_none() {
        println!("Something wrong with send")
    }
}

#[no_mangle]
pub unsafe extern "C" fn dispose_hello_service(tx: *mut ResponseHandler) {
    Box::from_raw(tx);
}

#[no_mangle]
pub unsafe extern "C" fn start_hello_server(
    builder: *mut ServerRuntimeBuilder,
    port: u16,
) -> *mut hello_service::RunningServer {
    Box::into_raw(Box::new(Box::from_raw(builder).run_server(port)))
}

#[no_mangle]
pub unsafe extern "C" fn terminate_hello_server(srvr: *mut hello_service::RunningServer) {
    Box::from_raw(srvr);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
