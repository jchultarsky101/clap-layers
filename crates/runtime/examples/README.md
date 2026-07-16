# clap-layers Examples

This directory contains example programs demonstrating different ways to use `clap_layers`.

## Example Index

| Example | Description |
|---------|-------------|
| [`cli_only.rs`](cli_only.rs) | Usage with only CLI arguments and defaults |
| [`environment_vars.rs`](environment_vars.rs) | Using environment variables for configuration |
| [`config_file.rs`](config_file.rs) | Loading configuration from TOML files |
| [`precedence_demo.rs`](precedence_demo.rs) | Demonstrating precedence order: CLI > env > file > default |
| [`sensitive_data.rs`](sensitive_data.rs) | Handling sensitive fields (passwords) that only come from CLI |
| [`dynamic_values.rs`](dynamic_values.rs) | Using `no_cli` for auto-generated values |
| [`complete_example.rs`](complete_example.rs) | Comprehensive example combining all features |

## Running Examples

```bash
# List all examples
cargo run --example <name>

# With environment variables (field names are lowercase)
MYAPP_port=8080 cargo run --example environment_vars

# With a config file (if required)
cargo run --example precedence_demo
```

**Note:** Environment variable names use the format `{PREFIX}_{FIELD_NAME}` where `FIELD_NAME` is **lowercase**. For example, with `env_prefix = "MYAPP"` and field `port`, the env var is `MYAPP_port`.

## Precedence Order

Configuration sources are applied in this order (highest to lowest priority):

1. **CLI flags** - `--port 8080`
2. **Environment variables** - `MYAPP_port=8080` (field name lowercase)
3. **Config file** - `config.toml`
4. **Defaults** - `default_value_t = 3000`

This means CLI always wins, followed by env vars (with lowercase field names), then config file, with built-in defaults as the fallback.
