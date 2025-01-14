#!/bin/bash

RUNTIME=${1?Either bajun or ajuna must be passed}

cargo build-"${RUNTIME}"-benchmarks

PALLETS=(
  "cumulus-pallet-xcmp-queue"
  "frame-system"
  "pallet-balances"
  "pallet-session"
  "pallet-timestamp"
  "pallet-collective"
  "pallet-membership"
  "pallet-treasury"
  "pallet-collator-selection"
  "pallet-multisig"
  "pallet-utility"
)

cd "$(git rev-parse --show-toplevel)" || exit
for PALLET in "${PALLETS[@]}"; do
  ./target/release/"${RUNTIME}"-para benchmark pallet \
    --chain=dev \
    --steps=50 \
    --repeat=20 \
    --pallet="${PALLET}" \
    --extrinsic="*" \
    --execution=wasm \
    --wasm-execution=compiled \
    --heap-pages=4096 \
    --header="./HEADER-AGPL" \
    --output="./runtime/${RUNTIME}/src/weights/${PALLET//-/_}.rs"
done
