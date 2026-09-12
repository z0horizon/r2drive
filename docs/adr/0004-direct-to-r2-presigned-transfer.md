# 0004: Direct-to-R2 PresignedTransfer Pipeline

WebConsole transfers (single and multipart uploads) will bypass the `StorageNode` data plane by generating presigned URLs via `r2kit`, allowing the browser to stream chunks directly to Cloudflare R2. This preserves server bandwidth, CPU, and memory, while enabling maximum client upload throughput for multi-gigabyte files.
