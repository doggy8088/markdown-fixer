# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--stdin` flag reads Markdown from standard input.
- `--exit-zero` for clipboard scripts.

### Fixed

-Column numbers for emoji are now reported in UTF-16 code units.
- Trailing whitespace inside fenced code is no longer touched.

## [1.2.0] - 2026-10-20

### Added

* New rule MDX042 for unpaired emphasis next to CJK punctuation.
* `--list-rules` prints the catalog.
- JSON output includes `wouldFix`.

### Changed

- Bumped MSRV to 1.85 .
- **Breaking:** `fix()` was renamed to `fix_document()`.

### Removed

- The `--legacy` flag. See #123 for details.

## [1.1.0] - 2026-07-02

### Security

- Refuse to write files that contain NUL bytes ([#98]).

# 1.0.0 - 2026-05-01

Initial release.

## [0.9.0-beta.1] - 2026-03-10

### Added
- Beta release for internal testing.
- 

[Unreleased]: https://github.com/example/markdown-fixer/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/example/markdown-fixer/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/example/markdown-fixer/compare/v1.0.0...v1.1.0
[0.9.0-beta.1]: https://github.com/example/markdown-fixer/releases/tag/v0.9.0-beta.1
