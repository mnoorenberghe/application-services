**See [the release process docs](docs/howtos/cut-a-new-release.md) for the steps to take when cutting a new release.**

# Unreleased Changes

[Full Changelog](https://github.com/mozilla/application-services/compare/v0.52.0...master)

## Android

### What's changed

- There is now preliminary support for an "autoPublish" local-development workflow similar
  to the one used when working with Fenix and android-components; see
  [this howto guide](./docs/howtos/locally-published-components-in-fenix.md) for details.

## FxA Client

### What's changed

- The `ensureCapabilities` method will not perform any network requests if the
  given capabilities are already registered with the server.
  ([#2681](https://github.com/mozilla/application-services/pull/2681)).