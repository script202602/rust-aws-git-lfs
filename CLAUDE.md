# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Prerequisites

- Rust (via rustup)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html) — required for building, local testing, and deploying

## Commands

```bash
# Build for production
cargo lambda build --release

# Build for development
cargo lambda build

# Run unit tests
cargo test

# Run a single test
cargo test test_generic_http_handler

# Start local Lambda server (hot-reload on file changes)
cargo lambda watch

# Invoke the local server with a pre-defined AWS event payload
cargo lambda invoke --data-example apigw-request

# Invoke with a custom JSON payload
cargo lambda invoke --data-file ./data.json

# Deploy to AWS (creates IAM role + Lambda function)
cargo lambda deploy
```

## Architecture

This is a minimal AWS Lambda HTTP handler using the `lambda_http` crate from the [aws-lambda-rust-runtime](https://github.com/awslabs/aws-lambda-rust-runtime) project.

- `src/main.rs` — entry point; initializes tracing and registers the handler with the Lambda runtime via `service_fn`
- `src/http_handler.rs` — contains `function_handler`, the core request/response logic, and its unit tests

The handler receives a `lambda_http::Request`, extracts query parameters, and returns a `Response<Body>`. All new business logic belongs in `http_handler.rs` (or additional modules imported from there). The `main.rs` wiring should not need to change.

HTTP integration tests use `cargo lambda watch` + `cargo lambda invoke` or direct `curl` against `localhost:9000`.
