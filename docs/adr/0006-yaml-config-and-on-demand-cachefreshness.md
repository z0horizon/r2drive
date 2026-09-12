# 0006: YAML Configuration and On-Demand CacheFreshness Synchronization

Adopt YAML (`config.yaml`) for human-editable file configuration, complemented by environment variables for container deployment. Bucket object indexing in `MetadataStore` operates on an on-demand basis with configurable TTL (`CacheFreshness`) plus user-triggered manual refresh, minimizing Cloudflare R2 Class B list operations and avoiding redundant background polling costs.
