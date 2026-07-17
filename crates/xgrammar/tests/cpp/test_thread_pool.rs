//! Port of `external/xgrammar/tests/cpp/test_thread_pool.cc`.

use std::{thread, time::Duration};

use xgrammar::support::ThreadPool;

#[test]
fn functional_test() {
    let mut pool = ThreadPool::new(4);

    // Example 1: Use Submit to submit tasks with return values
    let mut receivers = Vec::new();
    for i in 0..8 {
        let fut = pool.submit(move || {
            thread::sleep(Duration::from_millis(100));
            i * i
        });
        receivers.push(fut);
    }

    for fut in receivers {
        let _result = fut.recv().expect("task result");
    }

    // Example 2: Use Execute to submit tasks without return values
    for i in 0..5 {
        pool.execute(move || {
            thread::sleep(Duration::from_millis(50));
            let _ = i;
        });
    }

    // Wait for task to complete
    pool.join();
}

// TEST(XGramamrThreadPoolTest, PressureTest) remains commented out upstream.
