use log::trace;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
/*
use sentinel_macros::flow;
use sentinel_rs::utils::sleep_for_ms;
*/
#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_http_context(|context_id, _| -> Box<dyn HttpContext> {
        Box::new(FlowAuthorizer { context_id })
    });
}

struct FlowAuthorizer {
    context_id: u32,
}

impl Context for FlowAuthorizer {}

impl HttpContext for FlowAuthorizer {
    fn on_http_request_headers(&mut self, _: usize) -> Action {
        for (name, value) in &self.get_http_request_headers() {
            trace!("In WASM : #{} -> {}: {}", self.context_id, name, value);
        }

        match self.get_http_request_header("user") {
            // todo: use `wasm_bindgen`
            /*
            Some(token) if &token == "Cat" => match task() {
                Ok(_) => {
                    self.resume_http_request();
                    Action::Continue
                }
                Err(_err) => {
                    self.send_http_response(
                        429,
                        vec![("Powered-By", "proxy-wasm")],
                        Some(b"Too Many Requests.\n"),
                    );
                    Action::Pause
                }
            },
            _ => {
                self.send_http_response(
                    402,
                    vec![("Powered-By", "proxy-wasm")],
                    Some(b"Access forbidden.\n"),
                );
                Action::Pause
            }
             */
            Some(token) if &token == "Cat" => {
                self.resume_http_request();
                Action::Continue
            }
            _ => {
                self.send_http_response(
                    402,
                    vec![("Powered-By", "proxy-wasm")],
                    Some(b"Access forbidden.\n"),
                );
                Action::Pause
            }
        }
    }
}

/*
#[flow(
    calculate_strategy = "Direct",
    control_strategy = "Reject",
    threshold = 5.0
)]
fn task() {
    println!("{}: passed", sentinel_rs::utils::curr_time_millis());
    sleep_for_ms(10);
}
*/
