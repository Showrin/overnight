# Releasing

Releases are cut entirely by `.github/workflows/release.yml`.

```sh
git tag v0.2.0
git push origin v0.2.0
```

The workflow then:

1. Generates release notes from the commits since the previous tag with
   [git-cliff](https://git-cliff.org) (`cliff.toml`). Use conventional
   commit prefixes (`feat:`, `fix:`, `perf:`, `refactor:`, `docs:`) — they
   become the sections; `chore:`/`ci:`/`build:`/`test:`/`style:` are skipped.
2. Creates a draft release with those notes.
3. Builds Windows (NSIS + MSI), Linux (AppImage, deb, rpm) and macOS (Apple
   Silicon + Intel) in parallel, stamping the tag's version into
   `tauri.conf.json`, signing the updater bundles and uploading everything
   plus `latest.json` to the draft.
4. Publishes the release once every build succeeded.

A tag with a `-` (e.g. `v0.3.0-beta.1`) is published as a prerelease, which
the in-app updater ignores (it reads `releases/latest`).

## In-app updates

The app checks `releases/latest/download/latest.json` on launch and every 6
hours, and shows an "Update available" popout. Settings → Updates checks on
demand. Both show the notes of every release between the installed version
and the new one before installing.

On Linux only the AppImage self-updates; deb/rpm installs update through
their package manager.

## Signing key

Updates are verified against the public key in `tauri.conf.json`
(`plugins.updater.pubkey`). The private key lives outside the repo
(`~/.tauri/overnight.key`) and in the repo's Actions secrets:

- `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/overnight.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — empty unless the key was
  regenerated with a password

Losing the private key means already installed copies can never verify
another update — back it up.

## macOS signing and notarization

macOS builds are signed with a Developer ID Application certificate and
notarized through an App Store Connect API key. Actions secrets:

- `APPLE_CERTIFICATE` — base64 of the exported `.p12`
- `APPLE_CERTIFICATE_PASSWORD` — the `.p12` export password
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`
- `APPLE_API_ISSUER` — App Store Connect issuer ID
- `APPLE_API_KEY` — App Store Connect key ID
- `APPLE_API_KEY_P8` — contents of the `AuthKey_<KEYID>.p8` file
