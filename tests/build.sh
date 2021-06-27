#!/bin/bash

BINARY_PATH=$(cargo test --test integration --no-run --message-format=json | jq -r "select(.profile.test == true) | .filenames[]" | grep 'integration')
echo $BINARY_PATH
rm -f integration-binary
cp $BINARY_PATH integration-binary
