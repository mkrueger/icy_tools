# icy_net SSH Key Authentication Specification

## Status

Proposed contract between `icy_term` and the `icy_net` SSH client.

## Objective

`icy_net` must allow callers to authenticate an SSH connection with a private key, an SSH agent, or a password without requiring callers to use `russh` directly.

This specification covers client authentication only. Server host-key verification is defined separately below because accepting a client key and trusting a server key are independent operations.

## Current Behavior

The current SSH API accepts this credential structure:

```rust
pub struct Credentials {
    pub user_name: String,
    pub password: String,
    pub proxy_command: Option<String>,
}
```

`SSHConnection::open` passes the username and password to `authenticate_password`. It cannot select a private key, decrypt an encrypted key, use an SSH agent, or report which authentication methods the server permits.

## Required Public API

### Authentication configuration

`icy_net` should expose authentication as an explicit enum. Secret values should not implement `Debug` in a way that reveals their contents.

```rust
use std::path::PathBuf;

pub struct Credentials {
    pub user_name: String,
    pub authentication: SshAuthentication,
    pub proxy_command: Option<String>,
}

pub enum SshAuthentication {
    Password {
        password: SecretString,
    },
    PrivateKey {
        path: PathBuf,
        passphrase: Option<SecretString>,
    },
    Agent {
        public_key: Option<SshPublicKeySelector>,
    },
    Auto {
        private_keys: Vec<PrivateKeyCredential>,
        use_agent: bool,
        password: Option<SecretString>,
    },
}

pub struct PrivateKeyCredential {
    pub path: PathBuf,
    pub passphrase: Option<SecretString>,
}

pub enum SshPublicKeySelector {
    Fingerprint(String),
    PublicKeyFile(PathBuf),
}
```

`SecretString` may be an icy_net type or a suitable secrecy wrapper. At minimum, it must redact `Debug` output and zero its owned secret buffer on drop where practical.

The exact type names may follow icy_net conventions, but the API must represent each mode without interpreting an empty password or magic path as a mode selection.

### Backward compatibility

Existing password-based callers should have a low-friction migration path. One of the following is required:

1. Keep the existing `Credentials` constructor or fields temporarily and add a new `SshCredentials` type.
2. Add constructors such as `Credentials::password`, `Credentials::private_key`, and `Credentials::agent` and update all workspace callers in the same release.

A silent behavior change based on an empty `password` is not acceptable.

Suggested constructors:

```rust
impl Credentials {
    pub fn password(user_name: impl Into<String>, password: impl Into<String>) -> Self;

    pub fn private_key(
        user_name: impl Into<String>,
        path: impl Into<PathBuf>,
        passphrase: Option<SecretString>,
    ) -> Self;

    pub fn agent(user_name: impl Into<String>) -> Self;
}
```

## Authentication Behavior

### Private-key files

For `SshAuthentication::PrivateKey`, icy_net must:

1. Read the selected key file asynchronously or outside latency-sensitive executor work.
2. Parse OpenSSH private-key files supported by the pinned `russh`/`ssh-key` version.
3. Decrypt encrypted keys with the supplied passphrase.
4. Call the appropriate `russh` public-key authentication API.
5. return success only when the server accepts the key.
6. Release key and passphrase material as soon as the SSH session no longer needs it.

At minimum, Ed25519 and RSA keys should be supported when the underlying cryptographic backend supports them. ECDSA support should follow the algorithms enabled in icy_net's `russh` configuration.

PEM, PKCS#8, security-key-backed keys, and PuTTY PPK files are optional unless the pinned key parser already supports them. Unsupported formats must return a specific error rather than a generic authentication failure.

### Encrypted keys

An encrypted key without a passphrase must return `PassphraseRequired`. An incorrect passphrase must return `InvalidPassphrase` or an equivalent distinct error. icy_net must not prompt interactively; prompting belongs to icy_term.

### SSH agent

For `SshAuthentication::Agent`, icy_net should:

1. Connect to the platform's SSH agent using the standard environment or platform mechanism.
2. Enumerate available identities.
3. Optionally restrict authentication to a selected fingerprint or public-key file.
4. Try eligible identities until one succeeds or all fail.
5. Ask the agent to sign authentication data without exporting private-key material.

Agent support may initially be limited to Unix `SSH_AUTH_SOCK`, provided unsupported platforms return `AgentUnavailable` rather than falling back silently.

### Automatic mode

`SshAuthentication::Auto` must use a documented and deterministic order:

1. Explicitly configured private-key files, in caller-provided order.
2. SSH agent identities when `use_agent` is true.
3. Password when present and when the server permits password authentication.

Failures that mean "this credential was rejected" may advance to the next configured credential. Local errors such as an unreadable key, invalid key format, or missing passphrase must be reported and must not be hidden by password fallback unless the caller explicitly opts into ignoring such errors.

No authentication method may be attempted unless the caller configured it.

### Server method discovery

When supported by `russh`, icy_net should use the server's advertised remaining authentication methods to avoid impossible attempts. The final error should include the methods offered by the server, without including secrets.

## Host-Key Verification

The current client accepts every server host key. Adding client key authentication must not present this as a fully secure SSH configuration.

icy_net should expose a host-key policy independent of client authentication:

```rust
pub enum HostKeyPolicy {
    KnownHosts {
        path: PathBuf,
        accept_new: bool,
    },
    Fingerprint(SshHostKeyFingerprint),
    InsecureAcceptAny,
}
```

Required behavior:

- A changed key for an existing host must fail with `HostKeyMismatch`.
- An unknown key must fail unless `accept_new` or an explicit trust callback is enabled.
- `InsecureAcceptAny` must be explicitly selected and clearly named.
- Hostnames and non-default ports must be matched consistently with OpenSSH `known_hosts` conventions where practical.

Host-key verification may be delivered in a separate change, but the authentication API must leave room for it and must not conflate host trust with user authentication.

## Error Contract

SSH setup errors should be machine-readable so icy_term can show an appropriate dialog or request a passphrase. The error enum should distinguish at least:

```rust
pub enum SshAuthenticationError {
    KeyFileNotFound { path: PathBuf },
    KeyFileUnreadable { path: PathBuf, source: std::io::Error },
    UnsupportedKeyFormat { path: PathBuf },
    PassphraseRequired { path: PathBuf },
    InvalidPassphrase { path: PathBuf },
    AgentUnavailable,
    AgentHasNoIdentities,
    SelectedAgentKeyNotFound,
    AuthenticationRejected {
        attempted: Vec<SshAuthenticationMethod>,
        server_methods: Vec<SshAuthenticationMethod>,
    },
    HostKeyUnknown,
    HostKeyMismatch,
    Transport(Box<dyn std::error::Error + Send + Sync>),
}
```

The concrete representation can differ, but callers must be able to identify these conditions without parsing display strings.

Errors and logs must never contain passwords, key passphrases, private-key contents, agent signature data, or full serialized credentials.

## Connection API

`SSHConnection::open` may continue to accept credentials directly:

```rust
pub async fn open(
    addr: impl Into<String>,
    caps: TermCaps,
    credentials: Credentials,
) -> crate::Result<Self>;
```

For future extensibility, a single options structure is preferred:

```rust
pub struct SshConnectionOptions {
    pub credentials: Credentials,
    pub host_key_policy: HostKeyPolicy,
    pub connect_timeout: Duration,
    pub authentication_timeout: Duration,
}

pub async fn open_with_options(
    addr: impl Into<String>,
    caps: TermCaps,
    options: SshConnectionOptions,
) -> crate::Result<Self>;
```

The password-compatible `open` function can delegate to `open_with_options` during migration.

## icy_term Integration Requirements

Once icy_net provides this API, icy_term can add these per-address fields:

- Authentication mode: password, private key, SSH agent, or automatic.
- Private-key path.
- Optional selected agent-key fingerprint.
- Host-key policy or known-hosts path.

A private-key passphrase should not be serialized into the dialing-directory TOML by default. icy_term should request it when connecting or store it only through an operating-system credential service.

icy_term should construct the selected `SshAuthentication` variant and pass it unchanged to icy_net. Key parsing, agent protocol handling, signing, and server authentication negotiation remain owned by icy_net.

## Security Requirements

- Never log secrets or private-key data.
- Do not copy secret material more than necessary.
- Reject key files that cannot be parsed completely.
- Preserve restrictive file permissions when icy_net creates or updates known-hosts data.
- Do not automatically search every file in `~/.ssh` unless the caller selects automatic discovery.
- Do not silently downgrade from a configured key to password authentication.
- Keep `InsecureAcceptAny` available only as an explicit compatibility policy.
- Apply bounded timeouts to agent communication and authentication attempts.

## Test Requirements

### Unit tests

- Password credentials select password authentication.
- Unencrypted Ed25519 key loads and selects public-key authentication.
- Supported RSA key loads and selects public-key authentication.
- Encrypted key with the correct passphrase loads successfully.
- Encrypted key without a passphrase returns `PassphraseRequired`.
- Encrypted key with an incorrect passphrase returns `InvalidPassphrase`.
- Missing and unreadable files produce distinct errors.
- Unsupported key data returns `UnsupportedKeyFormat`.
- Secret-bearing types redact their `Debug` output.
- Automatic mode preserves configured authentication order.

Test keys must be generated for tests or committed as non-production fixtures clearly marked as test-only.

### Integration tests

Use an in-process or containerized SSH server to verify:

- Successful key-only login.
- Rejected key returns `AuthenticationRejected`.
- Agent login succeeds with a loaded identity.
- A selected agent fingerprint prevents unrelated identities from being tried.
- Automatic mode falls back only according to its configured policy.
- Password-only behavior remains compatible.
- Unknown and changed host keys follow the selected host-key policy.
- Authentication and agent operations respect timeouts.

## Acceptance Criteria

The icy_net work is complete when:

1. icy_term can supply a username and private-key path without reading or parsing the key itself.
2. Both encrypted and unencrypted supported OpenSSH keys authenticate successfully.
3. Missing passphrases and rejected keys are distinguishable by typed errors.
4. Existing password authentication continues to work.
5. No secret appears in logs or `Debug` output.
6. Agent authentication either works on documented platforms or returns a specific unsupported/unavailable error.
7. Authentication selection and fallback behavior are documented and covered by tests.
8. The API can be combined with a non-permissive host-key verification policy.

## Recommended Delivery Order

1. Introduce typed authentication configuration and errors while retaining password behavior.
2. Implement private-key file loading and public-key authentication.
3. Add encrypted-key passphrase support.
4. Add icy_term fields and passphrase prompting.
5. Add SSH-agent authentication.
6. Replace accept-any server-key behavior with configurable known-hosts verification.
