# bevy_vox_processor

An asset processor for the bevy engine that converts Magica Voxel `.vox` files into the `.glb` format.

### Installation

Add `bevy_vox_processor` to the Cargo.toml of your project, or run:

```sh
cargo add bevy_vox_processor
```

### Usage

For hot reloading, you need to add bevy's non-default `file_watcher` feature in your Cargo file (or a feature that includes it such as `dev`):

```toml
bevy = { version = "0.20.0", features = ["file_watcher"] }
```

Then, in your code:

```rust no_run
use bevy::prelude::*;
use bevy_vox_processor::VoxGltfProcessorPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                mode: AssetMode::Processed,// (1)
                ..default()
            }),
            VoxGltfProcessorPlugin, // (2)
        ))
        .add_systems(Startup, spawn_scene)
        .run();
}

fn spawn_scene(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset("knight.vox"));// (3)
    commands.spawn(WorldAssetRoot(scene));
    // Add cameras, lights
}
```
1. Enable `AssetMode::Processed` within your `DefaultPlugins`
2. Import and add the `VoxGltfProcessorPlugin`
3. Load a `.vox` file as a Gltf scene

The glb will automatically be generated in an `imported_assets` folder. If the `.vox` asset or its `.vox.meta` file is modified while your app is running, and bevy's `file_watcher` feature is enabled, the asset will be reprocessed and hot-reloaded.

#### `.meta` files

Optionally, you can add a `.meta` file to your assets folder containing import settings. To see the full set of possible values, you can use the supplied [command line tool](#vox-asset-meta). Note that you do not need to supply all the values, just the ones you want to override from their default values. eg, to just override the voxel size and add baked ambient occlusion:
```ron
(
    meta_format_version: "1.0",
    asset: Process(
        processor: "VoxToGlbProcessor",
        settings: (
            voxel_size: 0.1,
            texture_mode: Packed((
                ambient_occlusion: Some((
                    depth: 3,
                    intensity: 1.0,
                )),
            )),
        ),
    ),
)
```

## vox-asset-meta

This crate also contains a command-line tool, `vox-asset-meta`, that emits the default
processor metadata for a `.vox` file:

From the repo root, you can run this binary with:

```sh
cargo run -p bevy_vox_processor --bin vox-asset-meta -- assets/study.vox
```
Or, to simplify the command, install the binary first:
```sh
cargo install --path bevy_vox_processor --locked
```

Then you can run it directly and omit the `--` separator:
```sh
vox-asset-meta assets/knight.vox
```

Pass a directory to process its `.vox` files recursively. Files that already have a `.meta` sibling are skipped, so that nothing gets overwritten.

```sh
vox-asset-meta assets
```
