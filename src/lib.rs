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
                debug!("New watch event {:?}: {:?}", ev.kind, path);

                // NOTE: Ignore the error here as it just causes noise with a thread panic
                //       Instead, we'll catch the problem in the test assertion
                if let Err(x) = tx.send(path) {
                    debug!("[X] Channel closed: {}", x);
                }
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

        // Create N files within N directories and watch each directory
        for i in 1..=TOTAL_FILES {
            // Create a directory to house the file (parent path)
            let dir = tmp_path.join(format!("dir_{}", i));
            std::fs::create_dir_all(&dir).expect("Failed to create directory");
            let dir = dir
                .canonicalize()
                .expect("Failed to canonicalize parent path");

            // Create the file whose path we actually want to watch
            let path = dir.join("file");
            let _ = std::fs::write(&path, format!("Value {}", i))
                .unwrap_or_else(|_| panic!("Failed to write {:?}", path));
            debug!("[1] Creating {:?}", path);

            // Watch the parent path
            watcher
                .watch(&dir, notify::RecursiveMode::NonRecursive)
                .unwrap_or_else(|_| panic!("Failed to watch {:?}", dir));
            debug!("[2] Watching {:?}", dir);

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
            debug!("[4] Matched path {:?}", path);
            file_paths.remove(&path);
            std::thread::sleep(Duration::from_micros(10));
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
