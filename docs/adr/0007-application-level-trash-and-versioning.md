# 0007: Application-Level Soft-Delete and Object Versioning

Because Cloudflare R2 does not support native S3 bucket object versioning (`PutBucketVersioning` / `GetBucketVersioning` are unsupported), `r2drive` implements versioning and trash bin mechanics at the application layer. Deletions are soft-deleted via `deleted_at` timestamps in `MetadataStore`, while file overwrites trigger server-side `CopyObject` operations into a `.versions/` prefix tracked by the metadata store before replacing the live object.
