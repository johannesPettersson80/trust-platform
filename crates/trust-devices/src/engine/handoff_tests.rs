//! Native tests of the hand-over (design §1.5; specification 25 §2): a request that leaves gives a later answer
//! back to its job, while a request that the panel asks again keeps a later answer for its next wait. Each expected
//! value is the literal that the test hands over.

use std::thread;
use std::time::{Duration, Instant};

use super::Handoff;

#[test]
fn a_wait_that_runs_out_gives_the_later_answer_back_to_the_job() {
    // Design §1.5: an action whose request left without its answer delivers that answer in a later view, so the
    // hand-over gives it back to the job. A wrong implementation keeps it in the hand-over, where nobody takes it.
    let handoff = Handoff::open();
    assert_eq!(
        handoff.wait(Duration::from_millis(1)),
        None,
        "nothing has answered yet"
    );
    assert_eq!(handoff.finish(7), Err(7), "the answer goes back to the job");
}

#[test]
fn a_wait_again_that_runs_out_keeps_the_later_answer_for_the_next_wait() {
    // Specification 25 §2: a runtime's search asked again waits for the answer of the request already sent. A
    // wrong implementation gives the later answer back to the job, as a plain wait does, so the request asked
    // again never takes it, or it delivers the answer twice.
    let handoff = Handoff::open();
    assert_eq!(
        handoff.wait_again(Duration::from_millis(1)),
        None,
        "nothing has answered yet"
    );
    assert_eq!(
        handoff.finish(7),
        Ok(()),
        "the hand-over keeps the later answer"
    );
    assert_eq!(
        handoff.wait_again(Duration::ZERO),
        Some(7),
        "the next wait takes it"
    );
    assert_eq!(
        handoff.wait_again(Duration::ZERO),
        None,
        "an answer is taken once"
    );
}

#[test]
fn a_wait_again_takes_an_answer_that_comes_while_it_waits() {
    // Specification 25 §2: the request asked again waits at most 25 ms for the answer and takes it when it comes.
    // A wrong implementation misses an answer that comes during the wait, or waits out the whole wait regardless.
    let handoff = Handoff::open();
    let started = Instant::now();
    let (answer, finished) = thread::scope(|scope| {
        let job = scope.spawn(|| {
            thread::sleep(Duration::from_millis(20));
            handoff.finish(5)
        });
        let answer = handoff.wait_again(Duration::from_secs(10));
        (answer, job.join().ok())
    });
    assert_eq!(answer, Some(5), "the answer that came during the wait");
    assert_eq!(
        finished,
        Some(Ok(())),
        "the waiting request took the answer"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the wait ends when the answer comes"
    );
}
