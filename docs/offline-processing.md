# Offline voxel processing

`vox-to-gltf` converts MagicaVoxel `.vox` files to GLB before an application runs. An application that only loads the resulting GLB files needs Bevy's normal glTF support and does not need `bevy_vox_scene`, `vox_bake`, or the converter as runtime dependencies.

## Install from a local checkout

Install the command from this repository with:

```sh
cargo install --path vox-to-gltf
```

Cargo places `vox-to-gltf` in its normal binary directory. Re-run the command with `--force` after changing the local converter:

```sh
cargo install --path vox-to-gltf --force
```

During development of this workspace, the equivalent command without installation is:

```sh
cargo run -p vox-to-gltf -- assets/model.vox --output assets/generated/model.glb
```

## Convert an asset directory

Directory input is recursive and preserves source-relative paths:

```sh
vox-to-gltf assets/vox --output assets/generated
```

For example, `assets/vox/characters/robot.vox` becomes `assets/generated/characters/robot.glb`. Animated sources also produce one GLB: animated shapes contain sibling mesh nodes named `robot-001`, `robot-002`, and so on. The command checks the entire output set before writing and refuses to replace a file unless `--force` is supplied.

Generated GLBs can be committed to source control. This gives release builds no converter dependency and lets reviewers see when source assets change their deployed output. Run the converter in CI as a consistency check if both `.vox` and `.glb` files are committed.

## CI preprocessing

A simple CI job can install the converter from the checkout and generate assets into a clean directory:

```yaml
- name: Install voxel converter
  run: cargo install --path vox-to-gltf --locked

- name: Convert voxel assets
  run: vox-to-gltf assets/vox --output target/generated-assets
```

To verify committed generated files, convert into a temporary directory and compare it with the committed directory:

```yaml
- name: Check generated voxel assets
  run: |
    vox-to-gltf assets/vox --output target/generated-assets
    diff -rq assets/generated target/generated-assets
```

Pin the repository revision or published crate version used by CI. Identical input bytes and settings produce byte-identical GLBs, so a directory comparison detects converter, settings, or source changes.

## Build script preprocessing

A Cargo dev-dependency on `vox-to-gltf` does not make its executable callable from `build.rs`: dev-dependencies are for tests and examples, and this package exposes a binary rather than a library. A build script can instead use the reusable exporter from `vox_bake`:

```toml
[build-dependencies]
vox_bake = { version = "0.1", features = ["gltf"] }
```

```rust,no_run
use std::{env, fs, path::PathBuf};

fn main() {
    let source_path = PathBuf::from("assets/vox/robot.vox");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = fs::read(&source_path).unwrap();
    let export = vox_bake::export_vox_to_glb(
        &source,
        &vox_bake::VoxBakeSettings::default(),
    )
    .unwrap();
    fs::write(output_dir.join("robot.glb"), export.bytes).unwrap();
}
```

Build scripts should write generated files under `OUT_DIR`, declare every source and settings file with `cargo:rerun-if-changed`, avoid timestamps or environment-dependent settings, and fail the build when conversion fails. `vox_bake` and its optional glTF dependencies become build dependencies only and are not linked into the shipped application.

Cargo does not automatically copy arbitrary files from `OUT_DIR` into a Bevy asset directory. Projects therefore need a packaging step that copies those GLBs into their deployed asset tree or embeds them with `include_bytes!`. For most applications, an explicit preprocessing command or CI job is easier to inspect and cache than conversion inside `build.rs`.

## Runtime consumption

Enable Bevy's glTF support and load a generated file normally:

```rust,no_run
use bevy::prelude::*;

fn spawn(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(SceneRoot(assets.load(
        GltfAssetLabel::Scene(0).from_asset("generated/robot.glb"),
    )));
}
```

Animated voxel files contain sibling frame nodes beneath each animated shape's transform. Their names use one-based, zero-padded suffixes. `KHR_node_visibility` marks only frame one visible. Versioned, engine-neutral node extras record `vox_bake.animation.frame_count` on the common parent and `vox_bake.animation_frame.index` plus `initially_visible` on each frame. A Bevy post-processing system can use those values to initialize `Visibility` and attach its preferred flipbook component while Bevy's glTF loader does not yet handle the visibility extension directly. Playback speed is intentionally absent because MagicaVoxel does not provide one. The output contains no voxel data for runtime queries or remeshing. Cloud-volume voxels are rejected because glTF has no portable density-volume representation.

Run `vox-to-gltf --help` for all bake settings and their shared library defaults. The converter reports model, mesh, triangle, texture, and byte counts, followed by warnings for supported but lossy conversions.

## Publishing the crates

The packages must be published in dependency order because `vox-to-gltf` declares the registry version of `vox_bake` as well as its workspace path:

```sh
cargo publish -p vox_bake
# Wait until vox_bake 0.1.0 is visible in the crates.io index.
cargo publish -p vox-to-gltf
```

Before publishing, inspect and verify the library archive with:

```sh
cargo package -p vox_bake --list
cargo package -p vox_bake
```

`cargo package -p vox-to-gltf` cannot complete registry dependency resolution until the declared `vox_bake` version has been published. Its prospective archive can still be inspected with `cargo package -p vox-to-gltf --list`. After publishing `vox_bake`, run the full CLI package command and install its resulting archive before publishing the CLI.
