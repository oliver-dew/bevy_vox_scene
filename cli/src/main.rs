use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use bevy_vox_scene::{
    AmbientOcclusionSettings, TextureMipPolicy, TexturePackSettings, UnitOffset, VoxLoaderSettings,
    VoxelTextureMode, export_vox_to_glb,
};
use clap::{Command, CommandFactory, FromArgMatches, Parser, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum MeshOffsetArg {
    Zero,
    Center,
    CenterBase,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TextureModeArg {
    Packed,
    Palette,
}

#[derive(Debug, Parser)]
#[command(
    name = "vox-to-gltf",
    version,
    about = "Convert MagicaVoxel .vox scenes to binary glTF"
)]
struct Args {
    /// MagicaVoxel file or directory to convert.
    input: PathBuf,

    /// Output GLB path for a file, or output directory for a directory.
    /// Animated input adds -001, -002, … suffixes.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Replace existing output files.
    #[arg(long)]
    force: bool,

    /// Length of each side of a voxel.
    #[arg(long, value_name = "SIZE", value_parser = parse_positive_f32)]
    voxel_size: Option<f32>,

    /// Do not generate faces on the outside boundary of each model.
    #[arg(long)]
    no_mesh_outer_faces: bool,

    /// Position the mesh origin relative to its bounds.
    #[arg(long, value_enum)]
    mesh_offset: Option<MeshOffsetArg>,

    /// Multiplier applied to emissive materials.
    #[arg(long, value_name = "STRENGTH", value_parser = parse_non_negative_f32)]
    emission_strength: Option<f32>,

    /// Treat palette colors as linear rather than sRGB.
    #[arg(long)]
    no_srgb: bool,

    /// Roughness assigned to diffuse MagicaVoxel materials.
    #[arg(long, value_name = "ROUGHNESS", value_parser = parse_unit_f32)]
    diffuse_roughness: Option<f32>,

    /// Texture layout used for voxel materials.
    #[arg(long, value_enum)]
    texture_mode: Option<TextureModeArg>,

    /// Texels added around packed-atlas islands.
    #[arg(long, value_name = "TEXELS")]
    atlas_padding: Option<u32>,

    /// Round packed-atlas dimensions to a multiple of this value.
    #[arg(long, value_name = "TEXELS", value_parser = clap::value_parser!(u32).range(1..))]
    atlas_alignment: Option<u32>,

    /// Maximum packed-atlas width or height.
    #[arg(long, value_name = "TEXELS", value_parser = clap::value_parser!(u32).range(1..))]
    max_atlas_dimension: Option<u32>,

    /// Reserve packed-atlas padding/alignment for mip levels (0-4).
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(0..=4))]
    texture_max_lod: Option<u8>,

    /// Log packed-atlas size and occupancy statistics.
    #[arg(long)]
    atlas_diagnostics: bool,

    /// Bake voxel ambient occlusion with the given sampling depth (1-8).
    #[arg(long, value_name = "DEPTH", value_parser = clap::value_parser!(u8).range(1..=8))]
    ao_depth: Option<u8>,

    /// Strength of baked ambient occlusion. Used with --ao-depth.
    #[arg(
        long,
        value_name = "STRENGTH",
        requires = "ao_depth",
        value_parser = parse_ao_intensity
    )]
    ao_intensity: Option<f32>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args();
    let settings = loader_settings(&args)?;

    let jobs = conversion_jobs(&args.input, args.output.as_deref())?;
    let mut outputs = Vec::new();
    for (input, output) in jobs {
        let source = fs::read(&input)
            .map_err(|error| format!("could not read {}: {error}", input.display()))?;
        let exports = export_vox_to_glb(&source, &settings)
            .map_err(|error| format!("could not convert {}: {error}", input.display()))?;
        let paths = output_paths(&output, exports.len());
        outputs.extend(
            exports
                .into_iter()
                .zip(paths)
                .map(|(export, path)| (path, export.bytes)),
        );
    }

    if !args.force {
        let collisions = outputs
            .iter()
            .map(|(path, _)| path)
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        if !collisions.is_empty() {
            return Err(format!(
                "refusing to overwrite {}; pass --force to replace existing output",
                collisions[0].display()
            ));
        }
    }
    for (path, bytes) in outputs {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        fs::write(&path, &bytes)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), bytes.len());
    }
    Ok(())
}

fn parse_args() -> Args {
    Args::from_arg_matches(&cli_command().get_matches()).expect("Clap produced invalid arguments")
}

fn cli_command() -> Command {
    Args::command().after_help(defaults_help())
}

fn defaults_help() -> String {
    let settings = VoxLoaderSettings::default();
    let mesh_offset = if settings.mesh_offset == UnitOffset::ZERO {
        "zero"
    } else if settings.mesh_offset == UnitOffset::CENTER_BASE {
        "center-base"
    } else if settings.mesh_offset == UnitOffset::CENTER {
        "center"
    } else {
        "custom"
    };
    let texture = match settings.texture_mode {
        VoxelTextureMode::Packed(pack) => format!(
            "packed (padding {}, alignment {}, maximum dimension {}, mipmaps {}, diagnostics {})",
            pack.padding,
            pack.alignment,
            pack.max_dimension,
            match pack.mip_policy {
                TextureMipPolicy::Disabled => "disabled".to_string(),
                TextureMipPolicy::Limited { max_lod } => format!("through level {max_lod}"),
            },
            pack.log_diagnostics
        ),
        VoxelTextureMode::Palette => "palette".to_string(),
    };
    let ao = match settings.ambient_occlusion {
        Some(ao) => format!("depth {}, intensity {}", ao.depth, ao.intensity),
        None => format!(
            "disabled (default intensity when enabled: {})",
            AmbientOcclusionSettings::default().intensity
        ),
    };

    format!(
        "Defaults (from VoxelLoaderSettings):\n  voxel size: {}\n  mesh outer faces: {}\n  mesh offset: {}\n  emission strength: {}\n  sRGB colors: {}\n  diffuse roughness: {}\n  texture mode: {}\n  ambient occlusion: {}",
        settings.voxel_size,
        settings.mesh_outer_faces,
        mesh_offset,
        settings.emission_strength,
        settings.uses_srgb,
        settings.diffuse_roughness,
        texture,
        ao,
    )
}

fn loader_settings(args: &Args) -> Result<VoxLoaderSettings, String> {
    let mut settings = VoxLoaderSettings::default();
    if let Some(voxel_size) = args.voxel_size {
        settings.voxel_size = voxel_size;
    }
    if args.no_mesh_outer_faces {
        settings.mesh_outer_faces = false;
    }
    if let Some(offset) = args.mesh_offset {
        settings.mesh_offset = match offset {
            MeshOffsetArg::Zero => UnitOffset::ZERO,
            MeshOffsetArg::Center => UnitOffset::CENTER,
            MeshOffsetArg::CenterBase => UnitOffset::CENTER_BASE,
        };
    }
    if let Some(strength) = args.emission_strength {
        settings.emission_strength = strength;
    }
    if args.no_srgb {
        settings.uses_srgb = false;
    }
    if let Some(roughness) = args.diffuse_roughness {
        settings.diffuse_roughness = roughness;
    }

    if let Some(mode) = args.texture_mode {
        settings.texture_mode = match mode {
            TextureModeArg::Packed => VoxelTextureMode::Packed(TexturePackSettings::default()),
            TextureModeArg::Palette => VoxelTextureMode::Palette,
        };
    }
    let has_packed_options = args.atlas_padding.is_some()
        || args.atlas_alignment.is_some()
        || args.max_atlas_dimension.is_some()
        || args.texture_max_lod.is_some()
        || args.atlas_diagnostics;
    match &mut settings.texture_mode {
        VoxelTextureMode::Packed(pack) => {
            if let Some(padding) = args.atlas_padding {
                pack.padding = padding;
            }
            if let Some(alignment) = args.atlas_alignment {
                pack.alignment = alignment;
            }
            if let Some(max_dimension) = args.max_atlas_dimension {
                pack.max_dimension = max_dimension;
            }
            if let Some(max_lod) = args.texture_max_lod {
                pack.mip_policy = if max_lod == 0 {
                    TextureMipPolicy::Disabled
                } else {
                    TextureMipPolicy::Limited { max_lod }
                };
            }
            if args.atlas_diagnostics {
                pack.log_diagnostics = true;
            }
        }
        VoxelTextureMode::Palette if has_packed_options => {
            return Err("packed-atlas options cannot be used with --texture-mode palette".into());
        }
        VoxelTextureMode::Palette => {}
    }

    if matches!(settings.texture_mode, VoxelTextureMode::Palette) && args.ao_depth.is_some() {
        return Err("ambient occlusion requires --texture-mode packed".into());
    }
    settings.ambient_occlusion = args.ao_depth.map(|depth| {
        let defaults = AmbientOcclusionSettings::default();
        AmbientOcclusionSettings {
            depth: usize::from(depth),
            intensity: args.ao_intensity.unwrap_or(defaults.intensity),
        }
    });
    Ok(settings)
}

fn conversion_jobs(input: &Path, output: Option<&Path>) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let metadata = fs::metadata(input)
        .map_err(|error| format!("could not inspect {}: {error}", input.display()))?;
    if metadata.is_file() {
        return Ok(vec![(
            input.to_owned(),
            output
                .map(Path::to_owned)
                .unwrap_or_else(|| input.with_extension("glb")),
        )]);
    }
    if !metadata.is_dir() {
        return Err(format!(
            "input {} is neither a file nor a directory",
            input.display()
        ));
    }

    let output_root = output.unwrap_or(input);
    if output_root.exists() && !output_root.is_dir() {
        return Err(format!(
            "output {} must be a directory when the input is a directory",
            output_root.display()
        ));
    }

    let mut files = Vec::new();
    collect_vox_files(input, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!("no .vox files found in {}", input.display()));
    }

    files
        .into_iter()
        .map(|source| {
            let relative = source
                .strip_prefix(input)
                .map(Path::to_owned)
                .map_err(|error| {
                    format!(
                        "could not make {} relative to {}: {error}",
                        source.display(),
                        input.display()
                    )
                })?;
            Ok((source, output_root.join(relative).with_extension("glb")))
        })
        .collect()
}

fn collect_vox_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read directory {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("could not read entry in {}: {error}", directory.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_vox_files(&entry.path(), files)?;
        } else if file_type.is_file() && has_vox_extension(&entry.path()) {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn has_vox_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vox"))
}

fn output_paths(output: &Path, count: usize) -> Vec<PathBuf> {
    if count <= 1 {
        return vec![output.to_owned()];
    }
    let parent = output.parent().unwrap_or_else(|| Path::new(""));
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let extension = output.extension().unwrap_or_default().to_string_lossy();
    (1..=count)
        .map(|frame| {
            let suffix = format!("{frame:03}");
            let filename = if extension.is_empty() {
                format!("{stem}-{suffix}")
            } else {
                format!("{stem}-{suffix}.{extension}")
            };
            parent.join(filename)
        })
        .collect()
}

fn parse_ao_intensity(value: &str) -> Result<f32, String> {
    parse_non_negative_f32(value)
        .map_err(|_| "AO intensity must be a finite, non-negative number".to_string())
}

fn parse_positive_f32(value: &str) -> Result<f32, String> {
    let parsed = parse_finite_f32(value)?;
    if parsed > 0.0 {
        Ok(parsed)
    } else {
        Err("value must be greater than zero".into())
    }
}

fn parse_non_negative_f32(value: &str) -> Result<f32, String> {
    let parsed = parse_finite_f32(value)?;
    if parsed >= 0.0 {
        Ok(parsed)
    } else {
        Err("value must not be negative".into())
    }
}

fn parse_unit_f32(value: &str) -> Result<f32, String> {
    let parsed = parse_finite_f32(value)?;
    if (0.0..=1.0).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("value must be between 0 and 1".into())
    }
}

fn parse_finite_f32(value: &str) -> Result<f32, String> {
    let intensity = value
        .parse::<f32>()
        .map_err(|_| "value must be a number".to_string())?;
    if intensity.is_finite() {
        Ok(intensity)
    } else {
        Err("value must be finite".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, cli_command, conversion_jobs, loader_settings, output_paths};
    use bevy_vox_scene::{TextureMipPolicy, VoxLoaderSettings, VoxelTextureMode};
    use clap::Parser;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn animation_paths_are_one_based_and_padded() {
        assert_eq!(
            output_paths(Path::new("walk.glb"), 3),
            ["walk-001.glb", "walk-002.glb", "walk-003.glb"].map(PathBuf::from)
        );
    }

    #[test]
    fn ambient_occlusion_arguments_are_parsed() {
        let args = Args::try_parse_from([
            "vox-to-gltf",
            "model.vox",
            "--ao-depth",
            "4",
            "--ao-intensity",
            "0.75",
        ])
        .unwrap();
        assert_eq!(args.ao_depth, Some(4));
        assert_eq!(args.ao_intensity, Some(0.75));
    }

    #[test]
    fn ambient_occlusion_arguments_are_validated() {
        assert!(Args::try_parse_from(["vox-to-gltf", "model.vox", "--ao-depth", "9"]).is_err());
        assert!(
            Args::try_parse_from(["vox-to-gltf", "model.vox", "--ao-intensity", "1.0",]).is_err()
        );
    }

    #[test]
    fn omitted_options_retain_library_defaults() {
        let args = Args::try_parse_from(["vox-to-gltf", "model.vox"]).unwrap();
        let actual = loader_settings(&args).unwrap();
        let expected = VoxLoaderSettings::default();
        assert_eq!(actual.voxel_size, expected.voxel_size);
        assert_eq!(actual.mesh_outer_faces, expected.mesh_outer_faces);
        assert_eq!(actual.emission_strength, expected.emission_strength);
        assert_eq!(actual.uses_srgb, expected.uses_srgb);
        assert_eq!(actual.diffuse_roughness, expected.diffuse_roughness);
        assert_eq!(actual.texture_mode, expected.texture_mode);
        assert_eq!(actual.ambient_occlusion, expected.ambient_occlusion);
    }

    #[test]
    fn help_reports_library_defaults() {
        let help = cli_command()
            .try_get_matches_from(["vox-to-gltf", "--help"])
            .unwrap_err()
            .to_string();
        let defaults = VoxLoaderSettings::default();
        assert!(help.contains("Defaults (from VoxelLoaderSettings):"));
        assert!(help.contains(&format!("voxel size: {}", defaults.voxel_size)));
        assert!(help.contains(&format!(
            "emission strength: {}",
            defaults.emission_strength
        )));
        assert!(help.contains(&format!(
            "diffuse roughness: {}",
            defaults.diffuse_roughness
        )));
    }

    #[test]
    fn loader_setting_overrides_are_applied() {
        let args = Args::try_parse_from([
            "vox-to-gltf",
            "model.vox",
            "--voxel-size",
            "0.25",
            "--no-mesh-outer-faces",
            "--mesh-offset",
            "center-base",
            "--emission-strength",
            "3",
            "--no-srgb",
            "--diffuse-roughness",
            "0.4",
            "--atlas-padding",
            "2",
            "--atlas-alignment",
            "8",
            "--max-atlas-dimension",
            "4096",
            "--texture-max-lod",
            "3",
            "--atlas-diagnostics",
            "--ao-depth",
            "4",
        ])
        .unwrap();
        let settings = loader_settings(&args).unwrap();
        assert_eq!(settings.voxel_size, 0.25);
        assert!(!settings.mesh_outer_faces);
        assert_eq!(settings.emission_strength, 3.0);
        assert!(!settings.uses_srgb);
        assert_eq!(settings.diffuse_roughness, 0.4);
        assert_eq!(
            settings.ambient_occlusion.unwrap().intensity,
            bevy_vox_scene::AmbientOcclusionSettings::default().intensity
        );
        let VoxelTextureMode::Packed(pack) = settings.texture_mode else {
            panic!("expected packed textures")
        };
        assert_eq!(pack.padding, 2);
        assert_eq!(pack.alignment, 8);
        assert_eq!(pack.max_dimension, 4096);
        assert_eq!(pack.mip_policy, TextureMipPolicy::Limited { max_lod: 3 });
        assert!(pack.log_diagnostics);
    }

    #[test]
    fn palette_mode_rejects_packed_texture_options_and_ao() {
        let packed_option = Args::try_parse_from([
            "vox-to-gltf",
            "model.vox",
            "--texture-mode",
            "palette",
            "--atlas-padding",
            "2",
        ])
        .unwrap();
        assert!(loader_settings(&packed_option).is_err());

        let ao = Args::try_parse_from([
            "vox-to-gltf",
            "model.vox",
            "--texture-mode",
            "palette",
            "--ao-depth",
            "2",
        ])
        .unwrap();
        assert!(loader_settings(&ao).is_err());
    }

    #[test]
    fn directory_jobs_are_recursive_sorted_and_preserve_relative_paths() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("vox-to-gltf-{unique}"));
        let input = root.join("source");
        let output = root.join("generated");
        fs::create_dir_all(input.join("characters")).unwrap();
        fs::write(input.join("z.VOX"), []).unwrap();
        fs::write(input.join("characters/hero.vox"), []).unwrap();
        fs::write(input.join("ignored.txt"), []).unwrap();

        let jobs = conversion_jobs(&input, Some(&output)).unwrap();
        assert_eq!(
            jobs,
            [
                (
                    input.join("characters/hero.vox"),
                    output.join("characters/hero.glb")
                ),
                (input.join("z.VOX"), output.join("z.glb")),
            ]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
