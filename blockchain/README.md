## The **blockchain** program

The program workspace includes the following packages:
- `blockchain` is the package allowing to build WASM binary for the program and IDL file for it.  
  The package also includes integration tests for the program in the `tests` sub-folder
- `blockchain-app` is the package containing business logic for the program represented by the `BlockchainService` structure.  
- `blockchain-client` is the package containing the client for the program allowing to interact with it from another program, tests, or
  off-chain client.



[workspace]

members = ["client"]


[package]
name = "blockchain"
version = "0.1.0"
edition = "2024"

[dependencies]
blockchain-app = { path = "app" }
parity-scale-codec = { version = "3.6", features = ["derive"] }
scale-info = { version = "2.10", features = ["derive"] }

[build-dependencies]
blockchain-app = { path = "app" }
sails-rs = { version = "0.8.1", features = ["wasm-builder"] }
sails-idl-gen = "0.8.1"

[dev-dependencies]
blockchain = { path = ".", features = ["wasm-binary"] }
blockchain-client = { path = "client" }
sails-rs = { version = "0.8.1", features = ["gtest"] }
tokio = { version = "1.41", features = ["rt", "macros"] }

[features]
wasm-binary = []


# THESE ARE THE NEW PROBLEM LINES - they were not here in your previous root Cargo.toml
# You need to disable default features for them here as well
# parity-scale-codec = { version = "3.6", default-features = false, features = ["derive", "max-encoded-len"] } # Disable std
# scale-info = { version = "2.10", default-features = false, features = ["derive"] } # Disable std