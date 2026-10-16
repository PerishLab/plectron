use super::Config;
use crate::application::Application;
use crate::{Result, frames, integer, report};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub struct Startup {
    pub snapshot: Value,
    pub elapsed: u128,
    pub queries: usize,
    pub inspection: u128,
}

pub struct Capture {
    pub records: Vec<Value>,
    pub evicted: Vec<Value>,
    pub range: Value,
    pub snapshot: Value,
    pub trace: Value,
    pub startup: Startup,
}

pub struct Session<'a> {
    pub application: &'a Application,
}

struct Collection {
    cursor: i64,
    records: BTreeMap<i64, Value>,
    evicted: Vec<Value>,
}

impl Session<'_> {
    pub fn capture(
        &self,
        config: &Config,
        root: &Path,
        session: &mut Option<i64>,
    ) -> Result<Capture> {
        let startup = self.initial()?;
        frames::action(
            self.application,
            &startup.snapshot,
            "scenario.start(); 1;",
            Duration::ZERO,
        )?;
        let start = json!({"type": "start", "options": {}});
        let identity = integer(&self.application.observation(&start)?["session"], "session")?;
        *session = Some(identity);
        let action = if config.drive == "manual" {
            "interact with the Plectron window now".into()
        } else {
            format!("using {} display-path input", config.drive)
        };
        eprintln!(
            "feel: recording {:?} for {}s; {action}",
            config.label, config.seconds
        );
        let (mut records, evicted) = self.collect(
            config.seconds,
            Duration::from_secs_f64(config.interval / 1_000.0),
            &config.drive,
        )?;
        let range = self.evaluate("scenario.range();")?;
        let snapshot = self.drain(Duration::from_secs(2))?;
        self.refresh(&mut records)?;
        let trace = report::write(&self.application.export(identity)?, &root.join("analysis"))?;
        *session = None;
        Ok(Capture {
            records,
            evicted,
            range,
            snapshot,
            trace,
            startup,
        })
    }

    fn query(&self, command: Value) -> Result<Value> {
        Ok(self
            .application
            .request(&json!({"type": "query", "command": command}))?["reply"]
            .clone())
    }

    fn read(&self, identity: i64) -> Result<Value> {
        self.query(json!({"type": "read", "record": identity}))
    }

    fn evaluate(&self, source: &str) -> Result<Value> {
        let result = self
            .application
            .request(&json!({"type": "evaluate", "unit": 1, "source": source}))?;
        if result["type"] == "evaluation" {
            return Ok(result["display"].clone());
        }
        let identity = integer(&result["job"], "job")?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let snapshot = frames::page(self.application)?;
            let job = snapshot["execution"]["jobs"]
                .as_array()
                .and_then(|jobs| jobs.iter().find(|job| job["id"].as_i64() == Some(identity)))
                .ok_or("content job is absent")?;
            if job["phase"] == "complete" {
                return job["result"]
                    .get("Ok")
                    .cloned()
                    .ok_or_else(|| format!("content evaluation failed: {job}"));
            }
            thread::sleep(Duration::from_millis(2));
        }
        Err("content evaluation timed out".into())
    }

    fn initial(&self) -> Result<Startup> {
        let started = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match frames::wait(self.application, None, None, Duration::from_millis(2)) {
                Ok((snapshot, queries, inspection)) => {
                    return Ok(Startup {
                        snapshot,
                        elapsed: started.elapsed().as_nanos(),
                        queries,
                        inspection,
                    });
                }
                Err(error)
                    if error.contains("request handler did not reply")
                        && Instant::now() < deadline => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn collect(
        &self,
        seconds: f64,
        interval: Duration,
        drive: &str,
    ) -> Result<(Vec<Value>, Vec<Value>)> {
        let reply = self.query(json!({"type": "latest", "unit": 1}))?;
        let initial = selected(&reply).ok_or("initial frame record is absent")?;
        let mut collection = Collection {
            cursor: integer(&initial["id"], "initial record")?,
            records: BTreeMap::new(),
            evicted: Vec::new(),
        };
        let deadline = Instant::now() + Duration::from_secs_f64(seconds);
        let mut due = Instant::now();
        let midpoint = Instant::now() + Duration::from_secs_f64(seconds / 2.0);
        while Instant::now() < deadline {
            let now = Instant::now();
            if drive != "manual" && now >= due {
                let direction = if drive == "reversal" && now >= midpoint {
                    -1
                } else {
                    1
                };
                let wheel = json!({"type": "wheel", "unit": 1, "x": 360, "y": 360, "delta": direction * 80});
                self.application.request(&wheel)?;
                due += Duration::from_secs_f64(1.0 / 60.0);
            }
            self.advance(&mut collection)?;
            self.revisit(&mut collection.records)?;
            let delay = if drive == "manual" {
                interval
            } else {
                interval.min(due.saturating_duration_since(Instant::now()))
            };
            thread::sleep(delay);
        }
        for _ in 0..20 {
            if !self.revisit(&mut collection.records)? {
                break;
            }
            thread::sleep(interval);
        }
        let Collection {
            records,
            mut evicted,
            ..
        } = collection;
        evicted.sort_by_key(Value::as_i64);
        evicted.dedup();
        Ok((records.into_values().collect(), evicted))
    }

    fn advance(&self, collection: &mut Collection) -> Result<()> {
        let reply = self.query(json!({"type": "latest", "unit": 1}))?;
        let Some(latest) = selected(&reply) else {
            return Ok(());
        };
        let newest = integer(&latest["id"], "latest record")?;
        for identity in collection.cursor + 1..=newest {
            let reply = self.read(identity)?;
            match selected(&reply) {
                Some(current)
                    if current["target"]["type"] == "frame" && current["target"]["unit"] == 1 =>
                {
                    collection.records.insert(identity, current.clone());
                }
                Some(_) => {}
                None if reply["status"]["type"] == "evicted" => {
                    collection.evicted.push(json!(identity))
                }
                None => {}
            }
        }
        collection.cursor = collection.cursor.max(newest);
        Ok(())
    }

    fn revisit(&self, records: &mut BTreeMap<i64, Value>) -> Result<bool> {
        let pending = records
            .iter()
            .filter(|(_, value)| value["visible"].is_null())
            .map(|(identity, _)| *identity)
            .collect::<Vec<_>>();
        for identity in &pending {
            let reply = self.read(*identity)?;
            if let Some(current) = selected(&reply) {
                records.insert(*identity, current.clone());
            }
        }
        Ok(!pending.is_empty())
    }

    fn refresh(&self, records: &mut [Value]) -> Result<()> {
        for record in records {
            if record["state"]["type"] != "complete" || record["visible"].is_null() {
                let reply = self.query(json!({"type": "read", "record": record["id"]}))?;
                if let Some(current) = selected(&reply) {
                    *record = current.clone();
                }
            }
        }
        Ok(())
    }

    fn drain(&self, duration: Duration) -> Result<Value> {
        let deadline = Instant::now() + duration;
        let mut snapshot = frames::page(self.application)?;
        while !frames::settled(&snapshot) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
            snapshot = frames::page(self.application)?;
        }
        Ok(snapshot)
    }
}

fn selected(reply: &Value) -> Option<&Value> {
    (reply["status"]["type"] == "record").then(|| &reply["status"]["record"])
}
