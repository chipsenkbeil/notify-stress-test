use notify::{Error, Event, RecommendedWatcher};
use std::{path::PathBuf, sync::mpsc};

// Set to true to print out information during test
const DEBUG: bool = false;

macro_rules! debug {
    ($msg:expr $(, $args:expr)* $(,)?) => {
        if DEBUG {
            eprintln!($msg, $($args),*);
        }
    };
}

pub fn make_watcher() -> (RecommendedWatcher, mpsc::Receiver<PathBuf>) {
    let (tx, rx) = mpsc::channel();
    let watcher = notify::recommended_watcher(move |res: Result<Event, Error>| match res {
        Ok(ev) => {
            for path in ev.paths {
                debug!(
                    "New watch event {:?} for file: {:?}",
                    ev.kind,
                    path.file_name().unwrap()
                );

                // NOTE: Ignore the error here as it just causes noise with a thread panic
                //       Instead, we'll catch the problem in the test assertion
                let _ = tx.send(path);
            }
        }
        Err(x) => debug!("Watcher encountered error: {:?}", x),
    })
    .unwrap();
    (watcher, rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::Watcher;
    use std::collections::HashSet;
    use std::time::Duration;

    const TOTAL_FILES: usize = 1500;

    #[test]
    fn stress_test() {
        let (mut watcher, rx) = make_watcher();
        let tmp_path = std::env::temp_dir();
        let mut file_paths = HashSet::new();

        // Create 500 files and watch each of them
        for i in 1..=TOTAL_FILES {
            let path = tmp_path.join(format!("file_{}", i));
            let _ = std::fs::write(&path, format!("Value {}", i))
                .unwrap_or_else(|_| panic!("Failed to write {:?}", path));
            debug!("[1] Creating {:?}", path);

            let path = path.canonicalize().expect("Failed to canonicalize path");
            watcher
                .watch(&path, notify::RecursiveMode::NonRecursive)
                .unwrap_or_else(|_| panic!("Failed to watch {:?}", path));
            debug!("[2] Watching {:?}", path);

            file_paths.insert(path);
        }

        // Update all paths
        for path in file_paths.iter() {
            debug!("[3] Updating {:?}", path);
            let _ = std::fs::write(path, "new value");
        }

        // Sleep this thread to give the watcher a chance to catch up
        std::thread::sleep(Duration::from_secs(1));

        // Process all events to find modify events for file paths
        while let Ok(path) = rx.try_recv() {
            debug!("[4] New modify event for {:?}", path);
            debug!("[5] Matched file {:?}", path.file_name().unwrap());
            file_paths.remove(&path);

            // Pause a little bit to give a chance for more
            std::thread::sleep(Duration::from_millis(1));
        }

        // Assert that all paths had a modify event received
        assert_eq!(
            file_paths.len(),
            0,
            "{}/{} file paths not modified",
            file_paths.len(),
            TOTAL_FILES
        );
    }
}
