# Flux-Core Rust API

This repo provides a high-level Rust API for LLNL's [Flux's resource manager](https://flux-framework.org/), primarily
the [flux-core](https://github.com/flux-framework/flux-core) component.

## Dependencies

The most important dependency of the Flux-Core Rust API is `flux-core` version 0.71.0 or higher.
Earlier versions of `flux-core` are not supported due to certain functionality not being
present for `flux_reactor_t` (e.g., reference counting) that are critical to implementing the Rust API.

The Flux-Core Rust API has the following Rust dependencies (specified in `Cargo.toml`):
<table>
  <tr>
    <th>Crate Name</th>
    <th>Version</th>
    <th>Build Dependency?</th>
    <th>Required?</th>
    <th>Required Features</th>
    <th>Optional Features</th>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/semver">semver</a></td>
    <td>1.0</td>
    <td>✅</td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://github.com/flux-framework/flux-sys-rs">flux-sys</a></td>
    <td>0.3.0</td>
    <td></td>
    <td>✅</td>
    <td>core, idset, hostlist</td>
    <td>jobtap</td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/serde">serde</a></td>
    <td>1.0</td>
    <td></td>
    <td>✅</td>
    <td>derive</td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/serde_json">serde_json</a></td>
    <td>1.0</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/bitflags">bitflags</a></td>
    <td>2.8</td>
    <td></td>
    <td>✅</td>
    <td>serde</td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/libc">libc</a></td>
    <td>0.2</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/errno">errno</a></td>
    <td>0.3</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/thiserror">thiserror</a></td>
    <td>2.0</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/pastey">pastey</a></td>
    <td>0.2</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/chrono">chrono</a></td>
    <td>0.4</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/url">url</a></td>
    <td>2.5</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/indexmap">indexmap</a></td>
    <td>2.14.0</td>
    <td></td>
    <td>✅</td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/nix">nix</a></td>
    <td>0.31</td>
    <td></td>
    <td>✅</td>
    <td>fs</td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/tokio">tokio</a></td>
    <td>1.0</td>
    <td></td>
    <td></td>
    <td>net, rt, macros, time</td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/smol">smol</a></td>
    <td>2.0</td>
    <td></td>
    <td></td>
    <td></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="https://crates.io/crates/async-io">async-io</a></td>
    <td>2.0</td>
    <td></td>
    <td></td>
    <td></td>
    <td></td>
  </tr>
</table>

## Building and Using in Other Projects

As with most Rust crates, building flux-core-rs and using it in
other projects is as simple as adding the following to your
`dependencies` table in `Cargo.toml`:

```toml
[dependencies]
flux-core = { version = "0.1.0", git = "https://github.com/flux-framework/flux-core-rs.git", branch = "main" }
```

> [!NOTE]
> The `flux-core` crate is not yet available on crates.io.
> So, users need to add the dependency directly via `git`, as
> shown above.

In addition to a default build, flux-core-rs also provides the
following Cargo features for optional functionality:
* `jobtap`: builds the high-level Rust API for jobtap plugins
* `tokio`: builds `flux_core::async_driver::TokioDriver`, which enables driving the Tokio async runtime using the Flux reactor
* `smol`: builds `flux_core::async_driver::SmolDriver`, which enables driving the Smol async runtime using the Flux reactor

## Examples

TBA

## Copyright and License

The Flux-Core Rust API is distributed under the terms of the GNU Lesser General Public License v3.0. All new contributions must be made under this license.

See [LICENSE](LICENSE)
and [NOTICE.LLNS](NOTICE.LLNS)
for details.

SPDX-License-Identifier: LGPL-3.0
LLNL-CODE-XXXXXX
