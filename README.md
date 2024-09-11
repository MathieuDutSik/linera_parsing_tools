# linera_parsing_tools
Some tools and manuals related to linera-protocol

The usage of the system with `remote-net` is:

(A) Build the executables
```
cargo build --release --features metrics
```

(B) Run the `prometheus` tool with the `prometheus.yml` from this repository.

(C) Running the linera net up
```
./target/release/linera net up --policy-config devnet
```

(D) Assign the environment variables `LINERA_WALLET` and `LINERA_STORAGE` as inidicated from above.

(E) Run the tests with
```
cargo test -p linera-service --features metrics,remote-net
```

