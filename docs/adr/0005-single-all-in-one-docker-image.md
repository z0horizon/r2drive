# 0005: Single All-in-One Docker Image with Headless Mode Toggle

Distribute `r2drive` as a single unified Docker container embedding both `StorageNode` and `WebConsole` static assets, with an optional `--headless` flag (or `R2DRIVE_HEADLESS=true` environment variable) to run as an API-only service. This consolidates releases into a single tag (`ghcr.io/zer0horizon/r2drive:latest`) without fragmenting images or increasing CI build overhead.
