use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

pub enum ApplyEvent {
    Outputs(usize),
    Target { name: String, w: u32, h: u32, i: usize, n: usize },
    Cached(String),
    Render { name: String, w: u32, h: u32 },
    Progress { done: u32, total: u32 },
    Applied { name: String, skin: String },
    Done { skin: String, n: usize },
    Err(String),
    Exit(bool),
}

pub struct Applier {
    rx: Option<Receiver<ApplyEvent>>,
    pub running: bool,
}

impl Applier {
    pub fn new() -> Self {
        Self { rx: None, running: false }
    }

    /// Apply `skin` to all monitors (output=None) or one monitor (Some(name)).
    pub fn apply(&mut self, root: &Path, skin: &str, output: Option<&str>) {
        let mut args = vec![skin.to_string()];
        if let Some(o) = output {
            args.push("--output".to_string());
            args.push(o.to_string());
        }
        self.run(root, args);
    }

    /// Re-apply each monitor's saved skin (recreates the wallpaper surfaces to clear a
    /// compositor cross-monitor bleed). Does not change assignments; uses cached renders.
    pub fn refresh(&mut self, root: &Path) {
        self.run(root, vec!["--refresh".to_string()]);
    }

    fn run(&mut self, root: &Path, args: Vec<String>) {
        if self.running {
            return;
        }
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.running = true;
        let script = root.join("scripts/apply.sh");
        thread::spawn(move || {
            let mut cmd = Command::new("bash");
            cmd.arg(&script).args(&args);
            cmd.stdout(Stdio::piped()).stderr(Stdio::null());
            match cmd.spawn() {
                Ok(mut child) => {
                    if let Some(out) = child.stdout.take() {
                        for line in BufReader::new(out).lines().map_while(Result::ok) {
                            if let Some(ev) = parse(&line) {
                                let _ = tx.send(ev);
                            }
                        }
                    }
                    let ok = child.wait().map(|s| s.success()).unwrap_or(false);
                    let _ = tx.send(ApplyEvent::Exit(ok));
                }
                Err(e) => {
                    let _ = tx.send(ApplyEvent::Err(format!("spawn error: {e}")));
                    let _ = tx.send(ApplyEvent::Exit(false));
                }
            }
        });
    }

    pub fn poll(&mut self) -> Vec<ApplyEvent> {
        let mut v = vec![];
        if let Some(rx) = &self.rx {
            while let Ok(e) = rx.try_recv() {
                if let ApplyEvent::Exit(_) = &e {
                    self.running = false;
                }
                v.push(e);
            }
        }
        v
    }
}

fn parse(l: &str) -> Option<ApplyEvent> {
    let p: Vec<&str> = l.split_whitespace().collect();
    match p.first().copied() {
        Some("OUTPUTS") => Some(ApplyEvent::Outputs(p.get(1)?.parse().ok()?)),
        Some("TARGET") => Some(ApplyEvent::Target {
            name: p.get(1)?.to_string(),
            w: p.get(2)?.parse().ok()?,
            h: p.get(3)?.parse().ok()?,
            i: p.get(4)?.parse().ok()?,
            n: p.get(5)?.parse().ok()?,
        }),
        Some("CACHED") => Some(ApplyEvent::Cached(p.get(1)?.to_string())),
        Some("RENDER") => Some(ApplyEvent::Render {
            name: p.get(1)?.to_string(),
            w: p.get(2)?.parse().ok()?,
            h: p.get(3)?.parse().ok()?,
        }),
        Some("PROGRESS") => Some(ApplyEvent::Progress {
            done: p.get(1)?.parse().ok()?,
            total: p.get(2)?.parse().ok()?,
        }),
        Some("APPLIED") => Some(ApplyEvent::Applied {
            name: p.get(1)?.to_string(),
            skin: p.get(2)?.to_string(),
        }),
        Some("DONE") => Some(ApplyEvent::Done {
            skin: p.get(1)?.to_string(),
            n: p.get(2)?.parse().ok()?,
        }),
        Some("ERR") => Some(ApplyEvent::Err(p[1..].join(" "))),
        _ => None,
    }
}
