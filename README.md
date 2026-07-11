# 1bitshit_auto_loader-bkg

Auto-loader and orchestrator for the BitShit modular stack.

## Components

| Repo | Role |
|------|------|
| `1bitshit_kernel-bkg` | AVX2 ternary kernel + llama.cpp/GGML |
| `1bitshit_driver-bkg` | FFI bridge, dynamic lib loading, BitNet constructor |
| `1bitshit_engine-bkg` | Model detection, BitNet routing, RuntimeC selection |
| `1bitshit_cli-bkg` | Dashboard, model registry, skills UI |
| `1bitshit_models-bkg` | Model library metadata (registry + catalog) |

## Usage

### Bootstrap all components
```bash
cargo run --bin bitshit-auto-loader
```

### Install
```bash
./install.sh --backend auto
```

### Update
```bash
./update.sh
```

## Architecture

`1bitshit.auto` is the integration repo. This repo bootstraps and manages the 4 core modular repos:

- kernel → driver → engine → cli
