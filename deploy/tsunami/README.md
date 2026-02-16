# Tsunami Deployment Templates

These files are sanitized templates derived from the `tsunami` fork deployment setup.
They are intentionally isolated from the default `deploy/` scripts so they can be adopted incrementally.

## Goals

- Preserve useful `tsunami` operational additions (nginx, tor, electrs, optional delayed ord boot).
- Avoid hardcoded production hostnames and environment-specific values.
- Keep current default `deploy/*` behavior untouched.

## Placeholders To Replace

- `__ORD_DOMAIN__`: public hostname for your ord HTTP endpoint.
- `__ELECTRS_DOMAIN__`: public hostname for your electrs HTTP endpoint.
- `__ASSIGN_DOMAIN__`: public hostname for your assign-style API endpoint (if used).
- `__CHAIN__`: chain value passed to `ord` and `bitcoind` (`main`, `regtest`, `signet`, `test`, `testnet4`).
- `__CSP_ORIGIN__`: CSP origin for ord server, usually same as `__ORD_DOMAIN__`.
- `__ORD_PROXY_DOMAIN__`: upstream proxy domain for regtest `ord env --proxy` flows.

## Files

- `assign.service`: systemd unit template for an assign-style ord binary.
- `bitcoin.conf`: high-throughput + tor-aware bitcoind template.
- `bitcoind-regtest.service`: regtest helper unit template.
- `checkout`: checkout + setup bootstrap helper.
- `electrs.service`: systemd unit template for electrs.
- `nginx.conf.example`: reverse-proxy template for ord/electrs/assign.
- `ord.service`: ord server systemd unit template (HTTP on 8080).
- `setup`: idempotent setup script with optional deferred ord boot.
- `torrc`: minimal tor control-port template.

## Suggested Flow

1. Copy needed templates to target paths.
2. Replace placeholders with environment values.
3. Run `bash -n` on scripts before execution.
4. Run `./deploy/tsunami/setup <chain> <domain> <branch> <commit> [skip_boot_ord]`.
