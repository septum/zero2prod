#!/usr/bin/env bash

 # executed commands are printed to the terminal
set -x

# e - exit immediately on non zero status
# o pipefail -  prevents errors in a pipeline from being masked
set -eo pipefail

INGRESS=$(doctl apps list --no-header --format DefaultIngress | awk '{$1=$1};1')

until [[ $INGRESS != '' ]]
do
    >&2 echo "The ingress is not available - sleeping"
    sleep 5
    INGRESS=$(doctl apps list --no-header --format DefaultIngress | awk '{$1=$1};1')
done

>&2 echo "Ingress is $INGRESS"
