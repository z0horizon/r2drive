# 0002: Service Architecture, Multi-Bucket Profiles, and Dual-Mode CLI

`r2drive` will be built as a unified binary containing both the `StorageNode` REST backend service and standalone CLI commands. The service will support multi-bucket profiles allowing a single instance to manage multiple R2 buckets across accounts. The distribution strategy provides both a headless backend API service and an all-in-one Docker image embedding the `WebConsole`. Single-admin authentication (via token or password) secures administrative and WebConsole access for MVP.
