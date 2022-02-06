use log::trace;
use proxy_wasm::hostcalls::get_current_time;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;

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
            Some(token) if &token == "Cat" => {
                // sentinel_rs::system_metric::init_cpu_collector(1000); // would panic, same as follows
                // sentinel_rs::utils::sleep_for_ms(1000); // would panic since wasm did not support thread sleep
                let builder = sentinel_rs::EntryBuilder::new(token)
                    .with_traffic_type(sentinel_rs::base::TrafficType::Inbound);
                /*
                if builder.build().is_err() {
                    self.send_http_response(
                        429,
                        vec![("Powered-By", "proxy-wasm")],
                        Some(b"Too Many Requests.\n"),
                    );
                    return Action::Pause
                }
                */
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
