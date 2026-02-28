# Changelog

## [Unreleased]

### Added

- Added a log out button in management interface to revoke existing authentication.

### Changed

- Docker image now uses Google's ["distroless" base image](https://github.com/GoogleContainerTools/distroless).

### Removed

- Removed the `--multi-season` (`ANIFUNNEL_MULTI_SEASON`) flag. Multi-season matching is now always enabled.

## [2.0.0] - 2026-02-07

### Added

- Added an authentication mechanism to the management interface.
- Logging level can be controlled with `--log-level` / `ANIFUNNEL_LOG_LEVEL`.

### Changed

- **Breaking:** Management interface has been rewritten in React. Using the management interface now requires a modern browser with support for JavaScript.
- **Breaking:** Overrides are now saved in an SQLite database and persist between restarts. If you are using Docker, make sure to mount the database path on your local machine.
- Improved logging for Plex webhook handling.
- Episode number 1 is now also scrobbled if it matches the watching list. (#8)

### Removed

- **Breaking:** Removed authenticating with an Anilist API token as an argument or by using the `ANILIST_TOKEN` environment variable.
- Prebuilt Linux musl binaries are no longer available.

## [1.4.0] - 2024-12-27

### Added

- Added support for updating rewatching list entries. (#7)

### Changed

- Upgraded dependencies.

## [1.3.1] - 2024-07-25

### Fixed

- Fixed fuzzy matching crashing on Japanese text. (#6)

## [1.3.0] - 2024-07-11

### Added

- Option to filter webhooks by Plex username for multi-user servers.

### Changed

- Upgraded dependencies.
- Improved fuzzy matching logic.

### Fixed

- Fixed the management inteface crashing when the watching list is empty. (#4)

## [1.2.1] - 2024-02-24

### Changed

- Upgraded dependencies.

### Fixed

- Fixed rare `HTTP 413 Content Too Large` errors for unused Plex events. (#2)

## [1.2.0] - 2023-11-06

### Added

- Added a management interface to set title overrides and episode offsets.
- Notify users of an invalid token on startup.

### Changed

- Improved fuzzy matching logic for sequels.
- Upgraded dependencies.

## [1.1.0] - 2023-06-16

### Added

- Added `--multi-season` flag for matching multi-season shows in the Plex library.

### Fixed

- Fixed macOS binary downloads possibly being corrupted.

## [1.0.2] - 2023-04-26

### Changed

- Upgraded dependencies.

## [1.0.1] - 2022-12-12

### Added

- Additional Linux architectures binaries are available for releases.

### Changed

- Linux builds now use Rustls instead of OpenSSL.

## [1.0.0] - 2022-12-11

Initial release.

[Unreleased]: https://github.com/Hamuko/anifunnel/compare/2.0.0...HEAD
[2.0.0]: https://github.com/Hamuko/anifunnel/compare/1.4.0...2.0.0
[1.4.0]: https://github.com/Hamuko/anifunnel/compare/1.3.1...1.4.0
[1.3.1]: https://github.com/Hamuko/anifunnel/compare/1.3.0...1.3.1
[1.3.0]: https://github.com/Hamuko/anifunnel/compare/1.2.1...1.3.0
[1.2.1]: https://github.com/Hamuko/anifunnel/compare/1.2.0...1.2.1
[1.2.0]: https://github.com/Hamuko/anifunnel/compare/1.1.0...1.2.0
[1.1.0]: https://github.com/Hamuko/anifunnel/compare/1.0.2...1.1.0
[1.0.2]: https://github.com/Hamuko/anifunnel/compare/1.0.1...1.0.2
[1.0.1]: https://github.com/Hamuko/anifunnel/compare/1.0.0...1.0.1
[1.0.0]: https://github.com/Hamuko/anifunnel/releases/tag/1.0.0
