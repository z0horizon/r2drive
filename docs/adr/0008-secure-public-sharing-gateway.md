# 0008: Secure Public Sharing Gateway via Short-Lived Presigned Redirects

Public file sharing will use a capability gateway URL pattern (`/s/<share_token>`) verified by `StorageNode`. When recipients satisfy security gates (password check, expiration date, max download quotas), `StorageNode` generates a 5-minute ephemeral presigned R2 URL via `r2kit` and redirects (HTTP 302) the client for direct download. This preserves zero-egress server costs while enabling instant share revocation and password protection.
