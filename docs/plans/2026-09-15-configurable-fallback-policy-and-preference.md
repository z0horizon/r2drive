# Configurable FallbackPolicy and User FallbackPreference Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Provide multi-tier control over streaming proxy upload fallback via administrative `FallbackPolicy` in `config.yaml` (`proxy_fallback`, `max_proxy_file_size`), proactive discovery in `cors-probe`, dual-side `PayloadLimitGate` (HTTP 413), and user `FallbackPreference` in WebConsole (persisted in `localStorage`).

**Architecture:** 
- StorageNode introduces `TransfersConfig` in `config.yaml` supporting human-readable size limits (`"500MB"`, `"1GB"`) with default 5GB fallback.
- `GET /api/buckets/{profile}/cors-probe` broadcasts `fallback_policy: { enabled, max_payload_bytes }` to WebConsole during preflight discovery.
- `POST /api/buckets/{profile}/upload/proxy` strictly validates `Content-Length` at the door, rejecting disabled proxy with HTTP 403 and oversized payloads with HTTP 413 without consuming streaming I/O.
- WebConsole introduces a user preference toggle persisted in `localStorage`, verifies file size client-side against the server's limit, and gracefully falls back only when permitted by both user and server.

**Tech Stack:** Rust (Axum 0.8, Serde, Tokio), Svelte 5 runes (`$state`), TypeScript, Vitest, Tailwind CSS.

---

### Task 1: Backend `TransfersConfig` & `FallbackPolicy` in `src/config/`

**Files:**
- Modify: `src/config/model.rs`
- Modify: `src/config/mod.rs`
- Modify: `config.yaml.example`
- Test: `src/config/mod.rs` (unit tests)

**Step 1: Write the failing tests in `src/config/mod.rs`**
Add tests verifying:
- Default `Config` has `transfers.proxy_fallback == true` and `transfers.max_payload_bytes() == 5 * 1024 * 1024 * 1024`.
- Parsing YAML with `transfers: proxy_fallback: false, max_proxy_file_size: "250MB"` yields `proxy_fallback == false` and `max_payload_bytes() == 250 * 1024 * 1024`.
- Parsing YAML with human strings (`"1GB"`, `"500KB"`, `"1024"`) parses correctly.
- Invalid size string falls back to default 5GB gracefully.

**Step 2: Run tests to verify they fail**
Run: `cargo test --lib config::tests`

**Step 3: Implement `TransfersConfig` and size parser**
In `src/config/model.rs`:
```rust
fn default_proxy_fallback() -> bool {
    true
}

fn default_max_proxy_file_size() -> String {
    "5GB".to_string()
}

pub const DEFAULT_MAX_PROXY_PAYLOAD_BYTES: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransfersConfig {
    #[serde(default = "default_proxy_fallback")]
    pub proxy_fallback: bool,
    #[serde(default = "default_max_proxy_file_size")]
    pub max_proxy_file_size: String,
}

impl TransfersConfig {
    pub fn max_payload_bytes(&self) -> u64 {
        parse_size_str(&self.max_proxy_file_size).unwrap_or(DEFAULT_MAX_PROXY_PAYLOAD_BYTES)
    }
}
```
In `src/config/mod.rs`:
Implement `parse_size_str(input: &str) -> Option<u64>`.

**Step 4: Run tests to verify they pass**
Run: `cargo test --lib config::tests`

**Step 5: Commit**
```bash
git add src/config/ config.yaml.example
git commit -m "feat(config): add TransfersConfig with human-readable size parsing"
```

---

### Task 2: Backend `PayloadLimitGate` & Status Enforcement in `upload_proxy`

**Files:**
- Modify: `src/error.rs`
- Modify: `src/server/routes/transfers.rs`
- Test: `tests/api_test.rs`

**Step 1: Write the failing tests in `tests/api_test.rs`**
Add integration tests:
- When server has `proxy_fallback: false`, authenticated `POST /api/buckets/default/upload/proxy?key=test.txt` returns HTTP 403 Forbidden with error message `"Proxy upload fallback is disabled by server configuration"`.
- When server has `max_proxy_file_size: "10B"`, authenticated `POST` with `Content-Length: 11` returns HTTP 413 Payload Too Large with error message indicating size limit exceeded.

**Step 2: Run test to verify it fails**
Run: `cargo test --test api_test test_upload_proxy_limits`

**Step 3: Implement `PayloadLimitGate` in `src/server/routes/transfers.rs`**
In `src/error.rs`: Add `AppError::Forbidden(String)` -> StatusCode::FORBIDDEN, `AppError::PayloadTooLarge(String)` -> StatusCode::PAYLOAD_TOO_LARGE.
In `src/server/routes/transfers.rs`:
```rust
if !state.config.transfers.proxy_fallback {
    return Err(AppError::Forbidden(
        "Proxy upload fallback is disabled by server configuration".to_string(),
    ));
}

let max_bytes = state.config.transfers.max_payload_bytes();
if content_length > max_bytes {
    return Err(AppError::PayloadTooLarge(format!(
        "File size ({content_length} bytes) exceeds maximum proxy upload size limit of {max_bytes} bytes",
    )));
}
```

**Step 4: Run test to verify it passes**
Run: `cargo test --test api_test`

**Step 5: Commit**
```bash
git add src/error.rs src/server/routes/transfers.rs tests/api_test.rs
git commit -m "feat(api): enforce FallbackPolicy and PayloadLimitGate on proxy upload"
```

---

### Task 3: Backend Proactive Policy Broadcast via `cors-probe`

**Files:**
- Modify: `src/server/routes/transfers.rs`
- Test: `tests/api_test.rs`

**Step 1: Write the failing test in `tests/api_test.rs`**
Update `test_cors_probe_endpoint` to assert:
- JSON response includes `"fallback_policy"`:
  - `"enabled"`: boolean
  - `"max_payload_bytes"`: u64

**Step 2: Run test to verify it fails**
Run: `cargo test --test api_test test_cors_probe_endpoint`

**Step 3: Implement policy serialization in `cors_probe`**
In `src/server/routes/transfers.rs`:
```rust
Ok(Json(json!({
    "probe_url": probe_url,
    "fallback_policy": {
        "enabled": state.config.transfers.proxy_fallback,
        "max_payload_bytes": state.config.transfers.max_payload_bytes(),
    }
})))
```

**Step 4: Run test to verify it passes**
Run: `cargo test --test api_test test_cors_probe_endpoint`

**Step 5: Commit**
```bash
git add src/server/routes/transfers.rs tests/api_test.rs
git commit -m "feat(api): broadcast fallback_policy in cors-probe endpoint"
```

---

### Task 4: WebConsole API & Store Support (`FallbackPolicy` & `FallbackPreference`)

**Files:**
- Modify: `web/src/lib/api/transfers.ts`
- Modify: `web/src/lib/stores/bucket.svelte.ts`
- Modify: `web/src/lib/stores/upload.svelte.ts`
- Test: `web/src/lib/api/api.test.ts`
- Test: `web/src/lib/stores/stores.test.ts`

**Step 1: Write the failing tests in `web/src/lib/api/api.test.ts` and `stores.test.ts`**
- Test `getCorsProbe` returns `probe_url` and `fallback_policy`.
- Test `bucketStore.checkCors` stores `serverFallbackPolicy`.
- Test `uploadStore` (or `settingsStore`) manages `proxyFallbackPreference`:
  - Defaults to `true` if unset in `localStorage`.
  - Loads persisted value from `localStorage`.
  - Persists updates to `localStorage`.

**Step 2: Run tests to verify they fail**
Run: `pnpm --dir web test run src/lib/api/api.test.ts src/lib/stores/stores.test.ts`

**Step 3: Implement store and API models**
- In `web/src/lib/api/transfers.ts`:
  Export interface `ServerFallbackPolicy { enabled: boolean; max_payload_bytes: number; }`
  Update `getCorsProbe` to return `{ probe_url: string; fallback_policy: ServerFallbackPolicy }`.
- In `web/src/lib/stores/bucket.svelte.ts`:
  Add reactive rune `serverFallbackPolicy = $state<ServerFallbackPolicy>({ enabled: true, max_payload_bytes: 5368709120 })`.
- In `web/src/lib/stores/upload.svelte.ts`:
  Add reactive rune `proxyFallbackPreference = $state<boolean>(...)` with persistence.

**Step 4: Run tests to verify they pass**
Run: `pnpm --dir web test run src/lib/api/api.test.ts src/lib/stores/stores.test.ts`

**Step 5: Commit**
```bash
git add web/src/lib/api/transfers.ts web/src/lib/stores/
git commit -m "feat(web): add FallbackPolicy tracking and FallbackPreference persistence"
```

---

### Task 5: WebConsole UI Toggle & Client-Side `PayloadLimitGate`

**Files:**
- Modify: `web/src/lib/upload/worker.ts`
- Modify: `web/src/components/modals/CorsModal.svelte`
- Modify: `web/src/components/upload/UploadModal.svelte`
- Test: `web/src/lib/upload/upload.test.ts`

**Step 1: Write the failing tests in `web/src/lib/upload/upload.test.ts`**
- Test: When CORS fails but `proxyFallbackPreference` is false, upload fails with descriptive message without calling `uploadViaProxy`.
- Test: When CORS fails but server `fallback_policy.enabled` is false, upload fails with server disabled message.
- Test: When CORS fails and file exceeds `fallback_policy.max_payload_bytes`, upload fails early with size limit message.
- Test: When allowed, fallback succeeds as before.

**Step 2: Run tests to verify they fail**
Run: `pnpm --dir web test run src/lib/upload/upload.test.ts`

**Step 3: Implement UI toggle and client-side gate**
- In `web/src/lib/upload/worker.ts`:
  Add early checks before calling `executeProxyFallback`:
  Validate `proxyFallbackPreference`, `fallbackPolicy.enabled`, and `file.size <= fallbackPolicy.max_payload_bytes`.
- In `web/src/components/upload/UploadModal.svelte` & `CorsModal.svelte`:
  Add a clean toggle checkbox/switch for "Auto-fallback via server proxy when CORS is blocked".
  If `fallbackPolicy.enabled === false`, disable toggle with tooltip text.

**Step 4: Run tests to verify they pass**
Run: `pnpm --dir web test run`
Run: `pnpm --dir web check`

**Step 5: Commit**
```bash
git add web/src/lib/upload/worker.ts web/src/components/
git commit -m "feat(web): add client-side PayloadLimitGate and user fallback preference toggle"
```

---

### Task 6: End-to-End Verification & ROADMAP Milestone Completion

**Files:**
- Modify: `ROADMAP.md`

**Step 1: Run full verification suite**
Run: `./scripts/check.sh fast`
Run: `./scripts/check.sh test`

**Step 2: Mark ROADMAP items complete**
Update `ROADMAP.md` Milestone 1.5 to check `[x]` for:
- `[x] Configurable administrative FallbackPolicy in config.yaml`
- `[x] Proactive policy discovery via cors-probe and dual-side PayloadLimitGate`
- `[x] Client user FallbackPreference toggle in WebConsole`
Mark Milestone 1.5 complete: `[x] **1.5 CORS Diagnostic & Resilient Upload Fallback**`

**Step 3: Commit**
```bash
git commit -am "docs(roadmap): mark milestone 1.5 fully complete"
```
