use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn capture(backend: &str, output: &Path, scale: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_plectron"));
    command
        .args(["capture", backend])
        .arg(fixtures::root())
        .arg(output)
        .args(["--entry", "list/src/index.html"])
        .env_remove("HOME")
        .env_remove("PLECTRON_CONTROL_DIR");
    if let Some(scale) = scale {
        command.args(["--scale", scale]);
    }
    command.output().unwrap()
}

#[test]
fn export() {
    let directory = std::env::temp_dir().join(format!("plectron-capture-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let output = directory.join("cpu.ppm");
    let result = capture("cpu", &output, None);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let cpu = std::fs::read(&output).unwrap();
    assert!(cpu.starts_with(b"P6\n720 640\n255\n"));
    assert_eq!(cpu.len(), b"P6\n720 640\n255\n".len() + 720 * 640 * 3);
    assert!(
        !capture("unknown", &directory.join("invalid.ppm"), None)
            .status
            .success()
    );
    assert!(!directory.join("invalid.ppm").exists());
    compare(&directory, &cpu);
    std::fs::remove_dir_all(directory).unwrap();
}

fn compare(directory: &Path, cpu: &[u8]) {
    if cfg!(target_os = "macos") {
        let output: PathBuf = directory.join("metal.ppm");
        let result = capture("metal", &output, None);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read(output).unwrap(), cpu);
    }
    if cfg!(windows) {
        let output: PathBuf = directory.join("d3d12.ppm");
        let result = capture("d3d12", &output, None);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual = std::fs::read(output).unwrap();
        assert_eq!(actual.len(), cpu.len());
        let header = b"P6\n720 640\n255\n".len();
        assert_eq!(&actual[..header], &cpu[..header]);
        let maximum = actual[header..]
            .iter()
            .zip(&cpu[header..])
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .unwrap_or(0);
        assert!(maximum <= 1, "capture CPU/D3D12 channel diff {maximum}");
    }
}

#[test]
fn density() {
    let directory = std::env::temp_dir().join(format!("plectron-density-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for (scale, width, height) in [("2", 1440, 1280), ("3", 2160, 1920)] {
        let output = directory.join(format!("{scale}.ppm"));
        let result = capture("cpu", &output, Some(scale));
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(output).unwrap();
        let header = format!("P6\n{width} {height}\n255\n");
        assert!(bytes.starts_with(header.as_bytes()));
        assert_eq!(bytes.len(), header.len() + width * height * 3);
    }
    for scale in ["0", "-1", "NaN", "8", "wrong"] {
        let output = directory.join("invalid.ppm");
        assert!(!capture("cpu", &output, Some(scale)).status.success());
        assert!(!output.exists());
    }
    std::fs::remove_dir_all(directory).unwrap();
}
