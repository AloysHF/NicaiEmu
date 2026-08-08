# Contributing

## How to Contribute

1. **Fork** this repository
2. **Create** a feature branch (`git checkout -b feature/your-feature`)
3. **Commit** your changes (`git commit -m 'Add your feature'`)
4. **Push** to the branch (`git push origin feature/your-feature`)
5. **Open** a Pull Request

## Code Style

- Use English for all comments and documentation
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Prefer `anyhow::Result` for error handling
- Use `log` crate for logging (not `println!`)
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

## Areas That Need Help

- **Game compatibility testing** — test more games and report issues with screenshots
- **CBE format support** — improve parsing for different CBE variants
- **ARM CPU emulation** — improve instruction accuracy for edge cases
- **XSE VM execution** — implement missing script commands
- **Service bridge** — implement missing firmware services
- **Platform ports** — macOS, Linux testing and packaging
- **Documentation** — improve docs and code comments
- **Bug reports** — if you find a game that doesn't work correctly, please open an issue
- **Libretro integration** — complete the libretro core implementation

## Getting Started

Check the [open issues](https://github.com/jiangxincode/NicaiEmu/issues) for
tasks labeled `good first issue` or `help wanted`. If you have questions, feel
free to open a discussion issue.

To understand the CBE game file format, see
[Game File Formats](Game-File-Formats.md).

## Testing

Run the unit tests:

```bash
cargo test --workspace --release
```

For game compatibility testing, see [Game Compatibility](Game-Compatibility.md).
