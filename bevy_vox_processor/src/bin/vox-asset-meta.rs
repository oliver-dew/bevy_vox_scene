use bevy::{asset::processor::AssetProcessor, prelude::*, tasks::block_on};
use bevy_vox_processor::VoxGltfProcessorPlugin;
use std::{
    env::args_os,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut args = args_os().skip(1).filter(|arg| arg != "--");
    let Some(input) = args.next() else {
        eprintln!("You must supply an asset path as the first argument");
        return ExitCode::FAILURE;
    };

    if input == "--help" || input == "-h" {
        println!("Outputs default meta files for MagicaVoxel assets");
        println!("Usage: vox-asset-meta [OPTIONS] <INPUT>");
        println!("Arguments: <INPUT> .vox file or directory to process recursively");
        println!("Options: -h, --help      Display this help text");
        return ExitCode::SUCCESS;
    }

    if let Some(unexpected) = args.next() {
        eprintln!(
            "Unexpected extra argument: {}",
            unexpected.to_string_lossy()
        );
        return ExitCode::FAILURE;
    }

    let input = match fs::canonicalize(&input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Could not find {}: {error}", Path::new(&input).display());
            return ExitCode::FAILURE;
        }
    };

    let (asset_root, asset_paths) = if input.is_file() {
        if !is_vox_file(&input) {
            eprintln!("Input is not a .vox file: {}", input.display());
            return ExitCode::FAILURE;
        }

        (
            input
                .parent()
                .expect("a file has a parent directory")
                .to_owned(),
            vec![PathBuf::from(input.file_name().expect("a file has a name"))],
        )
    } else if input.is_dir() {
        let mut asset_paths = Vec::new();
        if let Err(error) = collect_vox_files(&input, &input, &mut asset_paths) {
            eprintln!("Could not read {}: {error}", input.display());
            return ExitCode::FAILURE;
        }
        asset_paths.sort();

        if asset_paths.is_empty() {
            println!("No .vox files found in {}", input.display());
            return ExitCode::SUCCESS;
        }

        (input, asset_paths)
    } else {
        eprintln!("Input is not a file or directory: {}", input.display());
        return ExitCode::FAILURE;
    };

    let mut app = App::new();

    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            mode: AssetMode::Processed,
            file_path: asset_root.to_string_lossy().into_owned(),
            ..default()
        },
        VoxGltfProcessorPlugin,
    ));

    let processor = app.world().resource::<AssetProcessor>().clone();
    let mut written = 0;
    let mut failed = 0;

    for asset_path in asset_paths {
        let source_path = asset_root.join(&asset_path);
        let meta_path = meta_path(&source_path);
        if meta_path.exists() {
            continue;
        }

        match block_on(processor.write_default_meta_file_for_path(asset_path)) {
            Ok(()) => {
                println!("Meta file written to {}", meta_path.display());
                written += 1;
            }
            Err(error) => {
                eprintln!("Could not write {}: {error}", meta_path.display());
                failed += 1;
            }
        }
    }

    if failed == 0 {
        if written > 1 {
            println!("{written} meta files written");
        }
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn collect_vox_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_vox_files(root, &path, files)?;
        } else if file_type.is_file() && is_vox_file(&path) {
            files.push(
                path.strip_prefix(root)
                    .expect("collected files are beneath the asset root")
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn is_vox_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("vox")
}

fn meta_path(source_path: &Path) -> PathBuf {
    let mut meta_path = OsString::from(source_path.as_os_str());
    meta_path.push(".meta");
    PathBuf::from(meta_path)
}
