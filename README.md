# Pizza Order Manager for Freenet

A decentralized pizza ordering application built on Freenet. Allows groups to collaboratively create and manage pizza orders with cryptographic verification and eventual consistency across peers.

[ » Live on freenet ](http://localhost:7509/v1/contract/web/HGC7cKPnCuHTeAUftUqKsWiWmYWSmxDNTxY21vK4nqjw/)

## Features

- Create and manage group pizza orders
- Each user can add their own order items
- Order creators can mark items as paid
- Cryptographically signed operations (ed25519)
- CRDT-based state synchronization for eventual consistency
- Dioxus-based reactive web UI

## Prerequisites

### Option 1: Using Nix (Recommended)

If you have [Nix](https://nixos.org/download.html) installed with flakes enabled:

```bash
# Enter development shell
nix develop

# Or with direnv (automatic)
direnv allow
```

This provides all dependencies including Rust, Dioxus CLI, and WASM tools.

### Option 2: Manual Installation

- [Rust](https://rustup.rs/) (latest stable)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started) for running the UI

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Dioxus CLI
cargo install dioxus-cli

# Add WASM target (for contract compilation)
rustup target add wasm32-unknown-unknown
```

## Quick Start

### Running the UI

```bash
# From project root
cd ui

# Development server with hot reload
dx serve

# Or build for production
dx build --release
```

The UI will be available at `http://localhost:8080`

### Running Tests

```bash
# Run all tests (contract + common)
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_merge_is_commutative

# Run tests for a specific crate
cargo test -p pizza-contract
cargo test -p pizza-common
```

### Building the Contract

```bash
# Build the contract for WASM
cargo build -p pizza-contract --target wasm32-unknown-unknown --release

# The WASM file will be at:
# target/wasm32-unknown-unknown/release/pizza_contract.wasm
```

### Type Checking

```bash
# Check all crates for errors
cargo check

# Check specific crate
cargo check -p pizza-ui
cargo check -p pizza-contract
```

## Development

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Watching for Changes

```bash
# UI with hot reload
cd ui && dx serve

# Or use cargo-watch for library changes
cargo install cargo-watch
cargo watch -x check
```

## Architecture

### Contract (Shared State)

The contract implements Freenet's `ContractInterface` trait:

- `validate_state` - Verify state integrity and signatures
- `update_state` - Apply state updates (commutative merge)
- `summarize_state` - Generate compact state summary
- `get_state_delta` - Compute differences for sync

### State Synchronization

Uses a CRDT-like pattern with:

1. **Version numbers** for last-writer-wins conflict resolution
2. **Cryptographic signatures** for authorization
3. **Delta-based sync** for efficiency

```rust
// State merging is commutative:
// merge(A, B) == merge(B, A)
state_a.merge(&state_b, &params)?;
```

### UI Components

| Component | Description |
|-----------|-------------|
| `App` | Main layout with sidebar and content area |
| `Sidebar` | List of orders with "New Order" button |
| `OrderView` | Order details, items table, paid status |
| `NewOrderDialog` | Modal for creating new orders |

## Contributing

### Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/pizza-freenet.git
   cd pizza-freenet
   ```
3. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Development Workflow

1. Make your changes
2. Ensure tests pass:
   ```bash
   cargo test
   ```
3. Format and lint:
   ```bash
   cargo fmt
   cargo clippy
   ```
4. Commit with a descriptive message:
   ```bash
   git commit -m "Add feature: description of changes"
   ```
5. Push and create a pull request

### Code Guidelines

- **Tests**: Add tests for new functionality, especially for state merging logic
- **Commutativity**: Any state merge operation must be commutative - add property-based tests
- **Signatures**: All state-modifying operations must be cryptographically signed
- **Documentation**: Add doc comments for public APIs

### Testing Guidelines

When adding new features, ensure:

1. **Unit tests** for individual functions
2. **Commutativity tests** for any merge operations:
   ```rust
   #[test]
   fn test_merge_is_commutative() {
       let result_ab = a.clone().merge(&b);
       let result_ba = b.clone().merge(&a);
       assert_eq!(result_ab, result_ba);
   }
   ```
3. **Delta round-trip tests** for synchronization:
   ```rust
   #[test]
   fn test_delta_round_trip() {
       let summary = state_a.summarize();
       let delta = state_b.delta(&summary);
       let mut reconstructed = state_a.clone();
       reconstructed.apply_delta(&delta);
       assert_eq!(reconstructed, state_b);
   }
   ```

### Areas for Contribution

- [ ] Real Freenet gateway integration (WebSocket)
- [ ] Delegate implementation for secure key storage
- [ ] Order sharing via invite links
- [ ] Order history and archiving
- [ ] Price currency support
- [ ] Mobile-responsive UI improvements
- [ ] Accessibility improvements
- [ ] Internationalization (i18n)

### Reporting Issues

When reporting issues, please include:

- Description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Rust version (`rustc --version`)
- OS and browser (for UI issues)

## License

MIT License - see LICENSE file for details.

## Resources

- [Freenet Documentation](https://docs.freenet.org/)
- [Dioxus Documentation](https://dioxuslabs.com/learn/0.6/)
- [River Chat](https://github.com/freenet/river) - Reference Freenet application
