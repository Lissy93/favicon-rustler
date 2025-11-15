# Favicon Rustler

A Cloudflare Workers service written in Rust that fetches and scales website favicons.

## Overview

This service accepts requests in the format `/{url}/{size}` and returns a favicon for the specified website, resized to the requested dimensions.

## Prerequisites

- **Rust**: Version 1.91.1 or later
- **Node.js**: Version 22.x or later
- **npm**: Version 10.x or later

## Development Setup

### 1. Install Dependencies

Install Node.js dependencies (includes Wrangler CLI):

```bash
npm install
```

Rust dependencies will be automatically installed when you build the project.

### 2. Build the Project

Check that the code compiles:

```bash
cargo check
```

Build for development:

```bash
cargo build
```

### 3. Local Development

Run the worker locally in development mode:

```bash
npm run dev
```

This will start a local server using Wrangler's development mode.

### 4. Deploy

Deploy to Cloudflare Workers:

```bash
npm run deploy
```

## Project Structure

- `src/lib.rs` - Main worker entry point and request handling
- `src/utils.rs` - Utility functions for finding and fetching favicons
- `Cargo.toml` - Rust dependencies and project configuration
- `package.json` - Node.js dependencies and scripts
- `wrangler.toml` - Cloudflare Workers configuration

## Dependencies

### Rust Crates

- `worker` (0.6.x) - Cloudflare Workers SDK for Rust
- `url` - URL parsing
- `soup` - HTML parsing
- `image` - Image manipulation and resizing
- `serde_json` - JSON parsing
- `wasm-bindgen` - WebAssembly bindings

### Node.js Packages

- `wrangler` - Cloudflare Workers CLI

## API Usage

Request format:
```
GET /{url-to-website}/{size}
```

Example:
```
GET /example.com/64
```

This will return a 64x64 PNG favicon for example.com.

**Constraints:**
- Maximum size: 512 pixels
- Returns PNG format
- Returns 404 if no icon found
- Returns 400 for invalid requests

## How It Works

1. Validates the request URL and size
2. Checks if the target website is accessible
3. Searches for favicon in multiple locations:
   - Apple touch icon
   - Standard favicon links
   - Web manifest
   - Well-known locations (favicon.ico, etc.)
   - OpenGraph images
   - Fallback to Google's favicon service
4. Fetches the icon and resizes it to the requested size
5. Returns the resized image as PNG

## License

See LICENSE file for details.
