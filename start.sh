#!/bin/bash

source $HOME/.cargo/env
cargo run --release &>> hub.log
