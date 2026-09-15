# r2drive

A self-hosted Cloudflare R2 drive service packaged as a Docker container, providing a web-based file management interface and a CLI, built on top of `r2kit`.

## Language

**r2drive**:
The unified binary executable and containerized service providing Cloudflare R2 storage management via WebConsole and CLI commands.
_Avoid_: App, tool, script

**WebConsole**:
The browser-based file management interface served by `r2drive` for browsing, uploading, downloading, and managing buckets and objects.
_Avoid_: WebUI, frontend dashboard, GUI

**StorageNode**:
The backend service providing REST APIs, authentication, and dispatching R2 operations via `r2kit`.
_Avoid_: Backend, server process, daemon

**BucketProfile**:
A named configuration entry specifying credentials, account ID, and bucket parameters for a managed R2 bucket.
_Avoid_: BucketConfig, AccountConfig, Connection

**AdminSession**:
An authenticated administrative session validated by token or password for controlling StorageNode and WebConsole.
_Avoid_: UserSession, Login

**MetadataStore**:
The persistence layer (initially SQLite, designed for PostgreSQL expansion) storing bucket metadata, indexed object entries, folder structures, and transfer state.
_Avoid_: Database, Cache, LocalStore

**PresignedTransfer**:
The direct browser-to-R2 upload/download coordination mechanism using presigned URLs generated via `r2kit` to offload network transfer from StorageNode.
_Avoid_: DirectUpload, S3Upload, StreamUpload

**HeadlessMode**:
An execution mode of `r2drive serve` that disables static WebConsole asset hosting and runs strictly as a headless API service.
_Avoid_: ApiOnly, BackendOnly, HeadlessServer

**CacheFreshness**:
The time-to-live policy and manual refresh mechanism governing when prefix listings in MetadataStore must re-synchronize with Cloudflare R2.
_Avoid_: Expiry, CacheTimeout, TTLSync

**TrashBin**:
The soft-deletion workflow retaining deleted objects in an archived state via `deleted_at` timestamps in MetadataStore prior to permanent purge.
_Avoid_: RecycleBin, DeletedItems

**FileVersion**:
An immutable historical snapshot of an object saved to the `.versions/` key prefix via server-side CopyObject prior to live file modification.
_Avoid_: ObjectVersion, Revision, Backup

**ShareToken**:
A secure, revocable capability identifier enabling public recipients to access a shared file subject to password and quota gates.
_Avoid_: ShareLink, PublicId, DownloadToken

**UserPrefix**:
The isolated R2 namespace (`users/<user_id>/...`) assigned to a specific tenant under multi-user mode.
_Avoid_: UserFolder, UserHome, TenantPath

**SelectiveSync**:
The client synchronization mechanism that maintains local lightweight placeholder stubs and hydrates file contents on-demand while resolving conflicts via branching copies.
_Avoid_: SmartSync, VirtualFiles, CloudSync

**GrpcService**:
The secondary high-throughput RPC protocol interface running alongside Axum REST, serving binary Protobuf requests for microservices and IPC daemons.
_Avoid_: GrpcServer, RpcInterface

**ResumableSession**:
A persistent multipart upload workflow tracking upload ID, part size, completed ETags, and missing chunks across network disconnects or browser reloads.
_Avoid_: UploadJob, ResumeState, TransferTask

**StaleUploadCleanup**:
The automated maintenance procedure aborting incomplete multipart uploads that have remained inactive beyond a 24-hour threshold to prevent storage waste.
_Avoid_: GarbageCollect, AbortJob, PartCleaner

**CorsProbe**:
The lightweight preflight health-check verification performed by WebConsole against Cloudflare R2 to determine if direct transfers are permitted by the bucket's CORS policy.
_Avoid_: CorsCheck, PreflightPing, CorsPing

**ProxyTransferFallback**:
The fail-safe streaming upload path routed through StorageNode when direct PresignedTransfer is blocked by browser CORS enforcement.
_Avoid_: ServerUpload, ProxyUpload, FallbackStream

**FallbackPolicy**:
The administrative configuration and capacity limits on StorageNode governing whether streaming proxy uploads are enabled and the maximum allowable payload size.
_Avoid_: ProxyConfig, FallbackRule, ServerUploadSetting

**FallbackPreference**:
The client-side user intent stored in WebConsole governing whether transfers should automatically divert to ProxyTransferFallback or halt with explicit CORS diagnostic guidance.
_Avoid_: UserSetting, AutoProxyToggle, ClientFallbackChoice

**PayloadLimitGate**:
The early boundary check on StorageNode and WebConsole evaluating payload size against FallbackPolicy before network streaming or bandwidth consumption occurs.
_Avoid_: SizeGuard, QuotaChecker, FileSizeValidator

