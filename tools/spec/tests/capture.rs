use spec::scenario::{self, Options};
use std::collections::BTreeSet;

#[test]
fn capture() {
    let directory = std::env::temp_dir().join(format!("plectron-capture-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let config = config::Store::default();
    config.set("scenario.count", 8.into());
    config.set("scenario.runs", 2.into());
    config.set("scenario.capture", true.into());
    config.set("scenario.normalized", true.into());
    config.set("scenario.output", directory.to_str().unwrap().into());
    let report = scenario::configured(
        "cards",
        Options {
            size: "small",
            mode: "--check",
            backend: "cpu",
            platform: ("test", "test"),
        },
        config,
    )
    .unwrap();
    let mut files = BTreeSet::new();
    for run in &report.runs {
        assert!(run.closed && run.failure.is_none());
        assert!(
            run.assets
                .contains_key("app://scenarios/benchmarks/normalize.css")
        );
        let seed = run.stages[0].capture.as_ref().unwrap();
        let steady = run.stages[1].capture.as_ref().unwrap();
        assert_eq!(seed.frame, steady.frame);
        assert_eq!(seed.file, steady.file);
        assert_eq!(seed.bytes, steady.bytes);
        assert_eq!(steady.encoding, 0);
        for stage in run.stages.iter().filter(|stage| stage.rendering.is_some()) {
            let capture = stage.capture.as_ref().unwrap();
            assert!(capture.ready >= capture.encoding + stage.total);
            let file = capture.file.as_ref().unwrap();
            assert!(files.insert(file));
            let data = std::fs::read(file).unwrap();
            assert_eq!(data.len(), capture.bytes);
            let mut reader = png::Decoder::new(std::io::Cursor::new(data))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!((info.width, info.height), (720, 640));
            assert_eq!(info.color_type, png::ColorType::Rgb);
            let checksum =
                pixels[..info.buffer_size()]
                    .chunks_exact(3)
                    .fold(0_u64, |hash, pixel| {
                        let value = u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]]);
                        hash.wrapping_mul(1099511628211) ^ u64::from(value)
                    });
            assert_eq!(checksum, stage.rendering.as_ref().unwrap().checksum);
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
