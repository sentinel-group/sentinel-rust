#!/usr/bin/env bash

cargo build --target=wasm32-unknown-unknown --release

cp ../../../target/wasm32-unknown-unknown/release/sentinel_envoy_module.wasm ./docker/sentinel_envoy_module.wasm

docker-compose up --build

pkt=1
while(( $pkt<10000 ))
do
    curl  -H "user":"Cat" 0.0.0.0:18000
    let "pkt++"
    sleep 0.001
done