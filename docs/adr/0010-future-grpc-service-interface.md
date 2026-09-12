# 0010: Future gRPC Service Interface alongside REST for Backend Integration

In Phase 3, `r2drive` will introduce a high-throughput gRPC interface implemented with `tonic` running alongside the Axum HTTP REST server. While REST remains the interface for browser-based `WebConsole` clients, gRPC provides low-latency binary serialization (Protobuf), bidirectional streaming, and type-safe contracts for backend-to-backend microservices, high-volume object synchronization, and local IPC communication for native daemons.
