use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use bevy_vox_scene::{AmbientOcclusionSettings, VoxLoaderSettings, export_vox_to_glb};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "vox-to-gltf",
    version,
    about = "Convert MagicaVoxel .vox scenes to binary glTF"
)]
struct Args {
    /// MagicaVoxel file to convert.
    input: PathBuf,

    /// Output GLB path. Animated input adds -001, -002, … suffixes.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Replace existing output files.
    #[arg(long)]
    force: bool,

    /// Bake voxel ambient occlusion with the given sampling depth (1-8).
    #[arg(long, value_name = "DEPTH", value_parser = clap::value_parser!(u8).range(1..=8))]
    ao_depth: Option<u8>,

    /// Strength of baked ambient occlusion. Used with --ao-depth.
    #[arg(
        long,
        value_name = "STRENGTH",
        default_value_t = 1.0,
        requires = "ao_depth",
        value_parser = parse_ao_intensity
    )]
    ao_intensity: f32,
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
    let args = Args::parse();
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("glb"));
    let source = fs::read(&args.input)
        .map_err(|error| format!("could not read {}: {error}", args.input.display()))?;
    let mut settings = VoxLoaderSettings::default();
    settings.ambient_occlusion = args.ao_depth.map(|depth| AmbientOcclusionSettings {
        depth: usize::from(depth),
        intensity: args.ao_intensity,
    });
    let exports = export_vox_to_glb(&source, &settings).map_err(|error| error.to_string())?;
    let paths = output_paths(&output, exports.len());
    if !args.force {
        let collisions = paths
            .iter()
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        if !collisions.is_empty() {
            return Err(format!(
                "refusing to overwrite {}; pass --force to replace existing output",
                collisions[0].display()
            ));
        }
    }
    for (export, path) in exports.iter().zip(&paths) {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        fs::write(path, &export.bytes)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), export.bytes.len());
    }
    Ok(())
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
    let intensity = value
        .parse::<f32>()
        .map_err(|_| "AO intensity must be a number".to_string())?;
    if intensity.is_finite() && intensity >= 0.0 {
        Ok(intensity)
    } else {
        Err("AO intensity must be a finite, non-negative number".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, output_paths};
    use clap::Parser;
    use std::path::{Path, PathBuf};

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
        assert_eq!(args.ao_intensity, 0.75);
    }

    #[test]
    fn ambient_occlusion_arguments_are_validated() {
        assert!(Args::try_parse_from(["vox-to-gltf", "model.vox", "--ao-depth", "9"]).is_err());
        assert!(
            Args::try_parse_from(["vox-to-gltf", "model.vox", "--ao-intensity", "1.0",]).is_err()
        );
    }
}
