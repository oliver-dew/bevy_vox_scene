# BSN migration and the `VoxelScene` blueprint

## Status

This document is an API design proposal, not an implementation plan for Bevy 0.20. BSN's Rust API is available in Bevy 0.20, but the `.bsn` asset format is not. The spelling of asset-valued props and some of the types below may therefore change once Bevy 0.21 establishes the file format.

Migrating the loader from `WorldAsset` to `ScenePatch` is useful, but it is not by itself a consumer-facing improvement. A cached scene can be patched at its root; BSN does not provide an external selector that reaches into an opaque scene and changes an arbitrary descendant. Names declared with `#Name` are scoped references used while resolving that scene, not public selectors for later patches.

The worthwhile API is a parameterised `VoxelScene` blueprint. It should make common instance-level policy declarative while keeping the generated hierarchy private.

## Migration trigger

Do not begin the production migration while Bevy's glTF loader still produces `WorldAsset`. Re-evaluate this proposal after a released Bevy version migrates glTF loading to BSN.

The glTF implementation should be treated as the reference architecture for the problems shared by both loaders:

- dynamically generated 3D hierarchies;
- meshes, materials, images, animation clips, and their asset dependencies;
- animation targets and cross-entity references;
- named scenes and labelled sub-assets;
- readiness, hot reload, and cached scene spawning;
- converting a non-BSN source format into runtime BSN scenes.

At that point, update this design to follow Bevy's public patterns unless a documented voxel-specific requirement justifies diverging. Also verify the released `.bsn` asset API before stabilizing `VoxelSceneProps` or `VoxelSceneSource`.

Until this trigger is met, retain `WorldAsset`, `WorldAssetRoot`, and `VoxelInstanceReady`. The API sketches below describe the desired consumer experience rather than work that should be implemented against provisional BSN internals.

## Goals

- Let a consumer include a `.vox` scene naturally inside a larger BSN hierarchy.
- Make common per-instance choices, especially flipbook playback, configurable without searching descendants or observing `VoxelInstanceReady`.
- Keep source loading and baking settings separate from per-instance scene policy.
- Preserve access to individual logical voxel models through `VoxelModelInstance` and `VoxelInstanceReady` for application-specific work.
- Support both asset-loaded `.vox` scenes and scenes created from generated voxel models.
- Give the library freedom to change the internal entity hierarchy without breaking consumers.
- Retain a useful runtime component that can be queried and patched after spawning.

## Non-goals

- Do not make every nested component remotely patchable. BSN composition does not turn an opaque hierarchy into a CSS-like selector tree.
- Do not expose internal entity names or frame entities as part of the public contract.
- Do not move bake settings such as texture packing, ambient occlusion, retained voxel data, or retained mesh data into per-instance scene fields. Those choices determine the assets that are produced and remain loader/generation settings.
- Do not replace `VoxelInstanceReady` for uncommon customization of a particular spawned model. The blueprint should eliminate the common uses of that event, not every possible use.

## Proposed consumer API

The common case should look approximately like this:

```rust,ignore
commands.spawn_scene(bsn! {
    #Enemy
    Transform::from_xyz(4.0, 0.0, -8.0)
    @VoxelScene {
        @source: "enemy.vox",
    }
});
```

`VoxelScene` is a `SceneComponent` placed on a stable wrapper entity. Its scene includes the cached scene produced by the `.vox` loader as a child. Transform, visibility, application markers, and observers can be composed normally on the wrapper.

Instance-level animation policy is expressed as ordinary, patchable fields:

```rust,ignore
commands.spawn_scene(bsn! {
    @VoxelScene {
        @source: "explosion.vox",
        flipbook: VoxelFlipbookSettings {
            repeat: Some(FlipbookRepeatMode::DespawnAfterCount(1)),
            frames_per_second: Some(18.0),
            playback: Some(VoxelPlayback::Playing),
        },
    }
});
```

The same blueprint composes as a child of a larger scene:

```rust,ignore
bsn! {
    #Vehicle
    VehicleController::default()
    Children [
        @VoxelScene {
            @source: "tank.vox#tank",
            flipbook: VoxelFlipbookSettings {
                repeat: Some(FlipbookRepeatMode::Forever),
                ..default()
            },
        }
        --
        #SelectionRing
        @selection_ring()
    ]
}
```

The exact asset-path conversion syntax is deliberately provisional. The important distinction is that `source` is a **prop**, while `flipbook` is a **component field**.

## Props versus fields

BSN evaluates `SceneComponent` props immediately to decide which scene to produce. Props are construction inputs and cannot subsequently be patched. Ordinary component fields participate in BSN's normal field-level patching and remain present at runtime.

`VoxelScene` should use that distinction deliberately:

```rust,ignore
#[derive(SceneComponent, Clone, Default)]
#[scene(VoxelSceneProps)]
pub struct VoxelScene {
    pub flipbook: VoxelFlipbookSettings,
}

pub struct VoxelSceneProps {
    pub source: VoxelSceneSource,
}
```

- `@source` is a prop because changing it means constructing a different hierarchy with different dependencies.
- `flipbook` is a normal field because it is instance policy, should be patchable through composition, and is useful to query or change at runtime.
- The scene function should copy any source identity needed for diagnostics into a private or read-only component rather than making the construction prop mutable after resolution.

The component should contain policy, not duplicated load settings. `VoxelScene` should not grow fields for `voxel_size`, atlas layout, ambient occlusion, `retain_voxel_data`, or `retain_mesh_data`.

### Reusable application blueprints

Because the resulting `VoxelScene` fields remain patchable, applications can build higher-level blueprints without exposing the source hierarchy:

```rust,ignore
fn explosion(source: VoxelSceneSource) -> impl Scene {
    bsn! {
        @VoxelScene {
            @source: source,
            flipbook: VoxelFlipbookSettings {
                repeat: Some(FlipbookRepeatMode::DespawnAfterCount(1)),
                ..default()
            },
        }
        PointLight {
            intensity: 4_000.0,
            ..default()
        }
    }
}

// Include the application blueprint, then patch one field for this instance.
bsn! {
    @explosion(explosion_asset)
    VoxelScene {
        flipbook: VoxelFlipbookSettings {
            frames_per_second: Some(24.0),
            ..default()
        },
    }
}
```

`VoxelScene`, `VoxelFlipbookSettings`, and any later settings types must support the appropriate BSN template derives so nested field patches compose rather than replace the complete settings value. This is the main blueprinting benefit: application scenes can layer policy on the public component while the library retains ownership of its descendants.

## Proposed public types

An initial API can stay intentionally small:

```rust,ignore
#[derive(SceneComponent, Clone, Default)]
#[scene(VoxelSceneProps)]
pub struct VoxelScene {
    pub flipbook: VoxelFlipbookSettings,
}

#[derive(Clone, Default)]
pub struct VoxelFlipbookSettings {
    /// `None` preserves the value authored by the source scene.
    pub repeat: Option<FlipbookRepeatMode>,
    /// `None` preserves the source scene's frame rate.
    pub frames_per_second: Option<f32>,
    /// `None` preserves the source scene's initial playback state.
    pub playback: Option<VoxelPlayback>,
}

#[derive(Clone, Copy, Default)]
pub enum VoxelPlayback {
    #[default]
    Playing,
    Paused,
}
```

Using `Option` is important. The wrapper's default should preserve the scene produced by the loader instead of silently overwriting authored or loader-configured values. Consumers opt into only the overrides they need.

`frames_per_second` is friendlier at the public boundary than exposing `FlipbookSpeed(Duration)`. Validation should reject or ignore non-finite and non-positive values consistently with `VoxBakeSettings::animation_frames_per_second`.

The first version should apply these settings to every descendant `FlipbookPlayer`. This covers the common case and has a clear meaning even if the source hierarchy changes.

### Possible later fields

Add fields only when they represent broadly useful instance policy:

- whether transform animation begins playing or paused;
- the initial transform-animation clip or graph policy;
- a material override applied to all surface models;
- visibility policy for source layers;
- collision or picking components installed on every logical model.

These should not be included speculatively. A small blueprint with good extension points is preferable to mirroring every nested Bevy component.

## Multiple animated models

A `.vox` scene may contain several independent animated shapes. The unqualified `flipbook` field applies to all of them.

Per-model overrides can be added later using stable MagicaVoxel paths rather than entity IDs or hierarchy positions:

```rust,ignore
@VoxelScene {
    @source: "factory.vox",
    flipbook: VoxelFlipbookSettings {
        playback: Some(VoxelPlayback::Paused),
        ..default()
    },
    model_overrides: vec![
        VoxelModelOverride::named("factory/fan").with_flipbook(
            VoxelFlipbookSettings {
                repeat: Some(FlipbookRepeatMode::Forever),
                playback: Some(VoxelPlayback::Playing),
                ..default()
            }
        ),
    ],
}
```

Override precedence would be:

1. Values produced by the source scene.
2. `VoxelScene`-wide fields.
3. The last matching named model override.
4. Runtime changes made by the application after readiness.

This facility should wait until there is demonstrated demand. Names may be absent or duplicated, so the API must define diagnostics and matching behavior before it is stabilized. Loading a named `.vox` subasset such as `"factory.vox#factory/fan"` is often a simpler answer.

## Scene shape and encapsulation

The blueprint should produce a hierarchy with one stable public root and private content beneath it:

```text
VoxelScene wrapper
├── VoxelScene
├── Transform / Visibility
├── internal source/readiness components
└── content root from the cached .vox ScenePatch
    └── arbitrary source hierarchy
        ├── VoxelModelInstance
        ├── FlipbookPlayer
        │   └── private FlipbookFrame entities
        └── other models and groups
```

Only the wrapper, `VoxelScene`, logical `VoxelModelInstance` entities, and documented readiness events are public structure. In particular, consumers must not depend on the content root existing as a separate entity or on the depth of a `FlipbookPlayer`.

Keeping a wrapper has several benefits:

- root transforms and application components never collide with the source root;
- a `VoxelScene` always has one predictable entity regardless of the `.vox` graph;
- policies can be queried and changed on one entity;
- replacing or hot-reloading the content does not replace the consumer-owned wrapper;
- readiness and diagnostics have an unambiguous target.

## Applying descendant policy

Composing a cached scene and patching `VoxelScene` does not automatically propagate its fields into descendants. `bevy_vox_scene` must own that propagation.

The practical initial implementation is:

1. The `VoxelScene` scene function installs the source scene and a private readiness observer on the wrapper.
2. BSN spawns the complete cached source hierarchy.
3. On BSN's bottom-up `Ready` event, the library reads `VoxelScene` from the wrapper.
4. It traverses only that wrapper's descendants and applies the configured policy to `FlipbookRepeatMode`, `FlipbookSpeed`, and `FlipbookPlayer`.
5. It emits `VoxelSceneReady` after all blueprint policy has been applied.
6. Existing per-model `VoxelInstanceReady` events are emitted for logical voxel instances as they are today.

This still uses a post-spawn observer internally, but consumers no longer write traversal or observer code. More importantly, the behavior becomes part of the `VoxelScene` contract and can later move to a more direct BSN template implementation without changing consumer syntax.

Changing a `VoxelScene` field at runtime should also reapply the relevant policy. This can be implemented with an `On<Insert, VoxelScene>`/change-detection system or explicit methods. It must not reset a player's current frame or timer unless the changed field requires it.

## Readiness events

BSN's `Ready` event is a low-level scene lifecycle event. The public voxel events should provide stronger ordering guarantees:

```rust,ignore
#[derive(EntityEvent)]
pub struct VoxelSceneReady {
    #[event_target]
    pub scene: Entity,
}
```

`VoxelSceneReady` means:

- all scene dependencies are loaded;
- the complete descendant hierarchy exists;
- `VoxelScene` policy has been applied;
- logical voxel instances can be queried;
- all `VoxelInstanceReady` events for the initial hierarchy have either been emitted already or will be emitted in a documented order.

Prefer one scene-level event plus the existing per-model event rather than exposing BSN observer ordering to consumers. Applications that only need to start, pause, or change repeat behavior should use `VoxelScene` fields and need no readiness observer.

## Asset-loaded and generated scenes

The source prop should accept a common source abstraction:

```rust,ignore
pub enum VoxelSceneSource {
    Asset(Handle<ScenePatch>),
    // The string-path form should resolve to the same handle during templating.
}
```

Both loader output and generated output should therefore be `ScenePatch` assets:

```rust,ignore
let source = voxel_assets.create_scene(model, palette, settings)?;

commands.spawn_scene(bsn! {
    @VoxelScene {
        @source: source,
        flipbook: VoxelFlipbookSettings {
            repeat: Some(FlipbookRepeatMode::Count(1)),
            ..default()
        },
    }
});
```

If Bevy 0.21 provides an asset-path template that works directly as a prop, `VoxelSceneSource` may be unnecessary. Avoid stabilizing a wrapper enum until the final BSN asset API is known.

`VoxelSceneAssets::create_scene` and `create_animation` should continue to receive `VoxelSceneSettings` for asset-production choices. Playback behavior belongs on the `VoxelScene` instance, not in `VoxelSceneSettings`:

```text
VoxBakeSettings       -> geometry/material/animation data
VoxLoaderSettings     -> loading plus retained CPU data
VoxelSceneSettings    -> generated asset plus retained CPU data
VoxelScene component  -> per-spawn instance behavior
```

## Direct scene inclusion

Advanced consumers may still want to include a raw voxel `ScenePatch` without the wrapper:

```rust,ignore
bsn! {
    :"tank.vox"
    Transform::from_xyz(1.0, 0.0, 0.0)
}
```

That should remain possible. It provides ordinary root-level BSN patching but no `VoxelScene` policy propagation or `VoxelSceneReady` guarantee. Documentation should present `@VoxelScene` as the recommended high-level API and raw scene inclusion as an escape hatch.

## Error handling and diagnostics

Invalid instance policy should not make an asynchronously spawned scene disappear without explanation.

- Asset parse/bake failures remain asset-loading errors.
- Invalid frame rates should produce a structured warning and preserve the source value, or be rejected by a constructor before spawning.
- A repeat/speed override on a scene with no `FlipbookPlayer` is a harmless no-op by default.
- A named override with no match should emit a warning once when the scene becomes ready.
- Duplicate matches should be deterministic and surfaced in diagnostics.
- Hot reload should reapply policy and emit readiness once for the new content generation, without accumulating observers.

## Migration path

The phases below start only after the migration trigger above is met.

### Phase 1: internal BSN spike

- Audit Bevy's BSN glTF loader and reuse its scene, dependency, animation-target, labelled-subasset, readiness, and hot-reload patterns.
- Convert one static model, one multi-model hierarchy, one flipbook animation, one transform animation, and one cloud scene into runtime `Scene`/`ScenePatch` values.
- Prove that animation entity references can be expressed without preallocating entities in a temporary `World`.
- Prove labelled sub-scenes such as `"study.vox#workstation/desk"` remain loadable.
- Measure whether typical Bevy 3D applications actually remove `bevy_world_serialization` from their final dependency tree.

### Phase 2: backend migration

- Make the `.vox` asset loader return `ScenePatch`.
- Make generated scene functions return `Handle<ScenePatch>`.
- Replace `WorldAssetRoot` examples with BSN/`ScenePatch` spawning.
- Adapt `VoxelInstanceReady` to BSN lifecycle events.
- Keep current runtime behavior otherwise unchanged.

### Phase 3: `VoxelScene` blueprint

- Add `VoxelScene`, `VoxelSceneProps`, and `VoxelFlipbookSettings`.
- Add the stable wrapper and internal policy application.
- Add `VoxelSceneReady` with explicit ordering semantics.
- Update examples to use `@VoxelScene` whenever they need per-instance behavior.
- Retain raw `ScenePatch` inclusion for advanced composition.

### Phase 4: `.bsn` integration

- Validate the API against Bevy's released `.bsn` file syntax.
- Ensure a `.bsn` scene can include a `.vox` source through `@VoxelScene`.
- Decide whether `vox-to-bsn` should emit references to external generated assets, reference the original `.vox`, or wait for Bevy's asset-in-BSN story. The presence of a `.bsn` parser alone does not solve serialization of generated meshes, images, materials, and animation clips.

## Acceptance examples

The design is successful when all of these require no descendant query or readiness observer in consumer code:

```rust,ignore
// Default scene.
@VoxelScene { @source: "prop.vox" }

// A one-shot generated effect.
@VoxelScene {
    @source: explosion,
    flipbook: VoxelFlipbookSettings {
        repeat: Some(FlipbookRepeatMode::DespawnAfterCount(1)),
        ..default()
    },
}

// The same cached source with different per-instance policies.
Children [
    @VoxelScene {
        @source: "torch.vox",
        flipbook: VoxelFlipbookSettings {
            frames_per_second: Some(6.0),
            ..default()
        },
    }
    --
    @VoxelScene {
        @source: "torch.vox",
        flipbook: VoxelFlipbookSettings {
            frames_per_second: Some(14.0),
            ..default()
        },
    }
]
```

Application-specific work on a particular logical model may still use `VoxelInstanceReady`:

```rust,ignore
bsn! {
    @VoxelScene {
        @source: "workstation.vox",
    }
    on(handle_workstation_model_ready)
}
```

That division is intentional: the blueprint owns common scene policy; readiness events remain the extensibility mechanism for behavior the library cannot predict.

## Open questions for Bevy 0.21

- Can a `SceneComponent` prop directly accept an asset path and resolve it as a cached `ScenePatch`, or is a custom source template required?
- Can a loaded cached scene be included as a child without adding an otherwise unnecessary content-root entity?
- What are the exact hot-reload semantics for a `ScenePatch` included by a `SceneComponent`?
- Can policy be fed into an unresolved child scene before it is cached, avoiding post-spawn propagation while preserving cache reuse?
- How should a custom scene loader expose labelled sub-scenes in the `.bsn` asset model?
- Will Bevy's glTF loader move to BSN in the same release, providing patterns for animation targets and generated asset dependencies?
- Does the final `.bsn` format support or plan to support generated asset definitions, or should `vox-to-bsn` always reference external mesh/image assets?
