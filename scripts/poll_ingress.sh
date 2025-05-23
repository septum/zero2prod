#!/usr/bin/env bash

INGRESS=$(doctl apps list --no-header --format DefaultIngress | awk '{$1=$1};1')
until [[ $INGRESS != '' ]]
do
    >&2 echo "The ingress is not available - sleeping"
    sleep 5
    INGRESS=$(doctl apps list --no-header --format DefaultIngress | awk '{$1=$1};1')
done
printf "%q" $INGRESS
>&2 echo "Ingress is $INGRESS"
