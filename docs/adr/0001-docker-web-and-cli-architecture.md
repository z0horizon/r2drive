# 0001: Deliver r2drive as a Docker image with WebConsole and CLI

Package `r2drive` as a self-hosted Docker container serving an embedded WebConsole for browser access while providing a unified CLI. We rejected native FUSE virtual drive mounting for v1 because FUSE requires kernel extensions (macFUSE) on macOS and platform-specific drivers that impede zero-friction adoption, whereas a containerized WebConsole allows users to deploy and manage Cloudflare R2 storage immediately on any platform with Docker.
