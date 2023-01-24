# Examples of how to use Sentinel

## Directory

### Embedded examples
- config: Show examples setting Sentinel configuration. 
- datasource: Dynamically define/load Sentinel rule by k8s objects, consul or etcd.   
- exporter: Examples on metric exporters, such as Prometheus.
- rules: All kinds of rules in Sentinel, together with `macros` features. It is recommended to start from `examples/rules/flow`.

### Individual examples
- ebpf: Demos on integrations with eBPF.
- proxy: Demos on integrations with proxies, such as Envoy.

## Testing

For embedded examples, simply run them with

```
cargo run --example $example_name
```

For ebpf related examples, visit its subdirectory.

The envoy proxy with WASM is not ready and only provide an experimental scaffold.  