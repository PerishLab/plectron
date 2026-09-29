use super::{fixture, json};
use serde_json::Value;
use spec::experiment::Document;

#[test]
fn samples() {
    let mut value = fixture();
    let run = value["runs"][0].clone();
    value["runs"] = json!([run.clone(), run.clone(), run]);
    value["repetitions"] = json!(3);
    for (run, times) in value["runs"].as_array_mut().unwrap().iter_mut().zip([
        [1, 1, 100],
        [1, 100, 1],
        [100, 1, 1],
    ]) {
        let stage = &mut run["stages"][3];
        stage["evaluation"] = json!(times[0] * 1000000_u64);
        stage["publication"] = json!(times[1] * 1000000_u64);
        stage["rendering"]["preparation"] = json!(times[2] * 1000000_u64);
        stage["total"] = json!(106000000);
    }
    let summary = Document::parse(value).unwrap().summary();
    let times = &summary.actions["edit"].timing;
    assert_eq!(times["cpu"].as_ref().unwrap().median, 102.0);
    assert_eq!(times["evaluation"].as_ref().unwrap().median, 1.0);
    assert_eq!(times["observed"].as_ref().unwrap().median, 106.25);
    assert_eq!(
        summary.actions["steady"].timing["cpu"]
            .as_ref()
            .unwrap()
            .median,
        4.0
    );
}
#[test]
fn incomplete() {
    let mut value = fixture();
    value["runs"][0]["stages"][3]["evaluation"] = Value::Null;
    value["runs"][1]["stages"][3]["rendering"]
        .as_object_mut()
        .unwrap()
        .remove("preparation");
    value["runs"][0]["stages"][3]["total"] = json!(u64::MAX);
    value["runs"][1]["stages"][3]
        .as_object_mut()
        .unwrap()
        .remove("inspection");
    let summary = Document::parse(value).unwrap().summary();
    assert!(summary.actions["edit"].timing["cpu"].is_none());
    assert!(summary.actions["edit"].timing["observed"].is_none());
}
