# Milestone 1.5: CORS Diagnostic & Resilient Upload Fallback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Provide proactive CORS preflight health detection and seamless streaming server-proxy upload fallback in `r2drive`, eliminating upload failures caused by Cloudflare R2 default cross-origin policies.

**Architecture:** WebConsole performs a lightweight `OPTIONS` preflight probe to a short-lived presigned URL (`GET /api/buckets/{profile}/cors-probe`) to detect CORS restrictions before uploading. If blocked, an amber warning banner and guided configuration modal with 1-click JSON copying guide the user. If direct uploads fail due to CORS, WebConsole seamlessly fails over to `POST /api/buckets/{profile}/upload/proxy`, which streams chunks with O(1) RAM from Axum directly to Cloudflare R2 via `aws-sdk-s3`/`r2kit` and updates `MetadataStore`.

**Tech Stack:** 
- Backend: Rust 1.94+ (Axum 0.8, Tokio, `r2kit 0.2.1`, `aws-sdk-s3`, SQLx)
- Frontend: Svelte 5 (TypeScript, Tailwind CSS, Lucide-Svelte, XMLHttpRequest)
- Database: Embedded SQLite (`MetadataStore`)
- Spec: [`docs/superpowers/specs/2026-09-12-cors-diagnostic-and-upload-fallback.md`](file:///Users/trungdt/Workspace/lib/r2drive/docs/superpowers/specs/2026-09-12-cors-diagnostic-and-upload-fallback.md)
- ADR: [`docs/adr/0012-cors-probe-and-proxy-transfer-fallback.md`](file:///Users/trungdt/Workspace/lib/r2drive/docs/adr/0012-cors-probe-and-proxy-transfer-fallback.md)

---

### Task 1: Backend `CorsProbe` Endpoint (`GET /api/buckets/{profile}/cors-probe`)

**Files:**
- Modify: `src/server/routes/transfers.rs`
- Modify: `src/server/mod.rs`
- Test: `tests/api_test.rs`

**Step 1: Write the failing test**

Add integration test in `tests/api_test.rs`:
```rust
#[tokio::test]
async fn test_cors_probe_endpoint() {
    let (app, _temp_dir) = spawn_test_app().await;

    // Unauthenticated request should be rejected with 401
    let unauth_req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/default/cors-probe")
        .body(Body::empty())
        .unwrap();
    let unauth_res = app.clone().oneshot(unauth_req).await.unwrap();
    assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);

    // Authenticated request with unknown bucket profile returns 404
    let auth_req = Request::builder()
        .method(Method::GET)
        .uri("/api/buckets/nonexistent/cors-probe")
        .header(header::COOKIE, "r2drive_session=valid_test_token")
        .body(Body::empty())
        .unwrap();
    let auth_res = app.oneshot(auth_req).await.unwrap();
    assert_eq!(auth_res.status(), StatusCode::NOT_FOUND);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test api_test test_cors_probe_endpoint`
Expected: FAIL with "status code 404 not matching 401" or route not found.

**Step 3: Write minimal implementation**

In `src/server/routes/transfers.rs`:
```rust
/// Handler for GET /api/buckets/{profile}/cors-probe
pub async fn cors_probe(
    State(state): State<AppState>,
    Path(profile): Path<String>,
) -> Result<Json<Value>, AppError> {
    let bucket = state.r2.get_bucket(&profile)?;
    let probe_url = bucket
        .presign_put("/.r2drive-probe", Duration::from_secs(60))
        .await
        .map_err(map_r2_error)?
        .into_url_string();

    Ok(Json(json!({
        "probe_url": probe_url,
    })))
}
```
In `src/server/mod.rs`, register `.route("/api/buckets/{profile}/cors-probe", get(transfers::cors_probe))` inside the authenticated router.

**Step 4: Run test to verify it passes**

Run: `cargo test --test api_test test_cors_probe_endpoint`
Expected: PASS

**Step 5: Commit**

```bash
git add src/server/routes/transfers.rs src/server/mod.rs tests/api_test.rs
git commit -m "feat(api): add GET /api/buckets/{profile}/cors-probe endpoint"
```

---

### Task 2: Backend Streaming `ProxyTransferFallback` (`POST /api/buckets/{profile}/upload/proxy`)

**Files:**
- Modify: `src/server/routes/transfers.rs`
- Modify: `src/server/mod.rs`
- Test: `tests/api_test.rs`

**Step 1: Write the failing test**

Add integration test in `tests/api_test.rs`:
```rust
#[tokio::test]
async fn test_upload_proxy_empty_key_rejected() {
    let (app, _temp_dir) = spawn_test_app().await;

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/buckets/default/upload/proxy?key=")
        .header(header::COOKIE, "r2drive_session=valid_test_token")
        .body(Body::from("test data"))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test api_test test_upload_proxy_empty_key_rejected`
Expected: FAIL (route not recognized or 404)

**Step 3: Write minimal implementation**

In `src/server/routes/transfers.rs`:
Define query struct:
```rust
#[derive(Debug, Deserialize)]
pub struct ProxyUploadQuery {
    pub key: String,
    #[serde(default)]
    pub content_type: Option<String>,
}
```
Handler implementation streaming `axum::body::Body` via `aws_sdk_s3::primitives::ByteStream` to R2:
```rust
pub async fn upload_proxy(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    Query(query): Query<ProxyUploadQuery>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Result<Json<Value>, AppError> {
    if query.key.trim().is_empty() {
        return Err(AppError::BadRequest("Object key cannot be empty".to_string()));
    }
    let bucket = state.r2.get_bucket(&profile)?;
    let content_length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    // Stream directly to R2 PutObject via aws_sdk_s3
    let sdk_client = bucket.client().as_sdk();
    let byte_stream = aws_sdk_s3::primitives::ByteStream::from_body_0_4(http_body_util::BodyStream::new(body));
    // ... Issue PutObject and insert ObjectRecord in state.db
    // Return Json response with object details
}
```
In `src/server/mod.rs`, register `.route("/api/buckets/{profile}/upload/proxy", post(transfers::upload_proxy))`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test api_test test_upload_proxy`
Expected: PASS

**Step 5: Commit**

```bash
git add src/server/routes/transfers.rs src/server/mod.rs tests/api_test.rs
git commit -m "feat(api): add streaming POST /api/buckets/{profile}/upload/proxy fallback endpoint"
```

---

### Task 3: WebConsole CORS Store & Proactive Preflight Probe

**Files:**
- Modify: `web/src/lib/api/transfers.ts`
- Modify: `web/src/lib/stores/bucket.svelte.ts`
- Test: `web/src/lib/stores/stores.test.ts`

**Step 1: Write the failing test**

In `web/src/lib/stores/stores.test.ts`:
```typescript
it('initializes corsStatus to unknown and transitions to healthy or blocked', async () => {
  const store = new BucketStore();
  expect(store.corsStatus).toBe('unknown');
  
  // Test checkCors transitioning to checking, then healthy on 200/OPTIONS success
  // or blocked on 403 / fetch error
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir web test run src/lib/stores/stores.test.ts`
Expected: FAIL (`corsStatus` undefined or property does not exist).

**Step 3: Write minimal implementation**

In `web/src/lib/api/transfers.ts`:
```typescript
export async function getCorsProbeUrl(profile: string): Promise<string> {
  const data = await apiRequest<{ probe_url: string }>(`/api/buckets/${profile}/cors-probe`);
  return data.probe_url;
}
```
In `web/src/lib/stores/bucket.svelte.ts`:
```typescript
export type CorsStatus = 'unknown' | 'checking' | 'healthy' | 'blocked';

// Inside BucketStore:
corsStatus = $state<CorsStatus>('unknown');

async checkCors(profile: string, force = false): Promise<void> {
  if (!force && this.corsStatus !== 'unknown') return;
  this.corsStatus = 'checking';
  try {
    const probeUrl = await getCorsProbeUrl(profile);
    const res = await fetch(probeUrl, {
      method: 'OPTIONS',
      headers: { 'Access-Control-Request-Method': 'PUT' },
    });
    if (res.ok || res.status === 200 || res.status === 204) {
      this.corsStatus = 'healthy';
    } else {
      this.corsStatus = 'blocked';
    }
  } catch {
    this.corsStatus = 'blocked';
  }
}
```
Call `checkCors(profile)` when bucket is selected.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir web test run src/lib/stores/stores.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add web/src/lib/api/transfers.ts web/src/lib/stores/bucket.svelte.ts web/src/lib/stores/stores.test.ts
git commit -m "feat(web): add corsStatus state and checkCors preflight probe in bucketStore"
```

---

### Task 4: WebConsole Amber Warning Banner & Smart Setup Modal

**Files:**
- Create: `web/src/components/layout/CorsBanner.svelte`
- Create: `web/src/components/modals/CorsModal.svelte`
- Modify: `web/src/App.svelte`

**Step 1: Write component definitions**

Create `CorsModal.svelte` with:
- Pre-filled JSON snippet with current `window.location.origin`:
  ```json
  [
    {
      "AllowedOrigins": ["http://localhost:8080", "*"],
      "AllowedMethods": ["GET", "PUT", "POST", "DELETE", "HEAD"],
      "AllowedHeaders": ["*"],
      "ExposeHeaders": ["ETag"],
      "MaxAgeSeconds": 3600
    }
  ]
  ```
- 1-Click "Copy CORS JSON" button with visual feedback tooltip.
- Direct external link to Cloudflare Dashboard.
- 3-step guide and "Re-check / Kiểm tra lại" action button.

Create `CorsBanner.svelte`:
- Display amber banner with alert icon if `bucketStore.corsStatus === 'blocked'`.
- "Xem hướng dẫn & cấu hình" button triggers modal.
- "Kiểm tra lại" button re-triggers `bucketStore.checkCors(profile, true)`.

**Step 2: Run Svelte check and Vitest**

Run: `pnpm --dir web check`
Expected: PASS (0 errors, 0 warnings)

**Step 3: Commit**

```bash
git add web/src/components/layout/CorsBanner.svelte web/src/components/modals/CorsModal.svelte web/src/App.svelte
git commit -m "feat(web): add CorsBanner and smart CorsModal with 1-click JSON copy"
```

---

### Task 5: WebConsole Resilient Upload Fallback (`uploadViaProxy`) & Seamless Retry

**Files:**
- Modify: `web/src/lib/api/transfers.ts`
- Modify: `web/src/lib/upload/worker.ts`
- Modify: `web/src/components/upload/UploadModal.svelte`
- Test: `web/src/lib/upload/upload.test.ts`

**Step 1: Write the failing test**

In `web/src/lib/upload/upload.test.ts`:
```typescript
it('falls back to uploadViaProxy when direct presigned upload throws CORS network error', async () => {
  // Mock direct fetch throwing TypeError: Failed to fetch
  // Expect uploadViaProxy called with XHR progress reporting and successful completion
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir web test run src/lib/upload/upload.test.ts`
Expected: FAIL (fallback logic not implemented).

**Step 3: Write minimal implementation**

In `web/src/lib/api/transfers.ts`, add `uploadViaProxy`:
```typescript
export function uploadViaProxy(
  profile: string,
  key: string,
  file: File,
  onProgress: (loaded: number, total: number) => void,
  signal?: AbortSignal
): Promise<any> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const url = `/api/buckets/${encodeURIComponent(profile)}/upload/proxy?key=${encodeURIComponent(key)}`;
    xhr.open('POST', url);
    xhr.withCredentials = true;
    if (file.type) xhr.setRequestHeader('Content-Type', file.type);
    
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(e.loaded, e.total);
    };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) resolve(JSON.parse(xhr.responseText));
      else reject(new Error(`Proxy upload failed with status ${xhr.status}`));
    };
    xhr.onerror = () => reject(new Error('Network error during proxy upload'));
    if (signal) signal.addEventListener('abort', () => xhr.abort());
    xhr.send(file);
  });
}
```
In `web/src/lib/upload/worker.ts`:
Catch direct PUT failure:
```typescript
if (isCorsOrNetworkError(err)) {
  item.fallback = true;
  return await uploadViaProxy(profile, item.key, item.file, onProgress, item.abortController.signal);
}
```
In `UploadModal.svelte`:
Add small badge `"Proxy fallback"` next to files uploading via server proxy.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir web test run src/lib/upload/upload.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add web/src/lib/api/transfers.ts web/src/lib/upload/worker.ts web/src/components/upload/UploadModal.svelte web/src/lib/upload/upload.test.ts
git commit -m "feat(web): support transparent streaming server-proxy upload fallback on CORS error"
```

---

### Task 6: Full Verification & Documentation

**Files:**
- Modify: `ROADMAP.md`
- Run: `./scripts/check.sh fast`
- Run: `./scripts/check.sh test`

**Step 1: Check off Milestone 1.5 in `ROADMAP.md`**
Update `ROADMAP.md` marking 1.5 items as completed `[x]`.

**Step 2: Execute full test verification**
Run:
```bash
./scripts/check.sh fast
./scripts/check.sh test
```
Expected: PASS with 100% clean test results.

**Step 3: Commit**
```bash
git add ROADMAP.md
git commit -m "docs: mark milestone 1.5 complete in ROADMAP"
```
