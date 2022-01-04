#!/usr/bin/env bash

pkt=1
while(( $pkt<10000 ))
do
    curl  -H "user":"Cat" 0.0.0.0:18000
    let "pkt++"
    sleep 0.001
done