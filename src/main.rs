//! A demonstration of future cancelation in Rust, showing that `await!` may
//! never return.

#![feature(await_macro, async_await, futures_api)]

#[macro_use]
extern crate tokio_async_await;

use failure::{self, format_err, ResultExt};
use std::{
    future::Future as StdFuture,
    result,
    time::{Duration, Instant},
};
use tokio::{prelude::*, timer::Delay};
use tokio_async_await::compat;

/// Let's abbreviate `Result` handling to something easy.
type Result<T> = result::Result<T, failure::Error>;

fn main() -> Result<()> {
    let mut runtime = tokio::runtime::Runtime::new()
        .context("unable to create a runtime")?;
    runtime.block_on(tokio_fut(wait_for_first_task()))?;
    Ok(())
}

/// Create a `delay` future that will pause for `milliseconds`
/// when we `await!` on it.
fn delay(milliseconds: u64) -> Delay {
    Delay::new(
        Instant::now()
            + Duration::from_millis(milliseconds),
    )
}

/// An asynchronous function that completes quickly.
async fn quick_task() -> Result<&'static str> {
    println!("START quick_task");
    await!(delay(10)).context("delay failed")?;
    println!("END quick_task");
    Ok("quick_task result")
}

/// An asynchronous function that completes very slowly.
async fn slow_task() -> Result<&'static str> {
    println!("START slow_task");
    await!(delay(10_000)).context("delay failed")?;
    println!("END slow_task");
    Ok("slow_task result")
}

/// A structure which prints `msg` on `drop`.
struct Cleanup {
    msg: &'static str,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        println!("{}", self.msg)
    }
}

/// An asynchronous function that cleans up after itself.
async fn protected_slow_task() -> Result<&'static str> {
    println!("START protected_slow_task");
    let _cleanup = Cleanup {
        msg: "CLEANUP protected_slow_task",
    };
    await!(delay(10_000)).context("delay failed")?;
    println!("END protected_slow_task");
    Ok("protected_slow_task result")
}

/// Run our tasks in parallel, waiting for the first to complete.
async fn wait_for_first_task() -> Result<()> {
    let all_futures = vec![
        boxed_fut(quick_task()),
        boxed_fut(slow_task()),
        boxed_fut(protected_slow_task()),
    ];
    let (result, _idx, others) = await!(
        future::select_all(all_futures)
    )
    .map_err(|(err, idx, others)| {
        println!("A future failed, so throw the rest away");
        drop(others);
        format_err!("task {} failed: {}", idx, err)
    })?;
    println!("Result from the first task: {}", result);
    println!("Dropping unneeded computations");
    drop(others);

    Ok(())
}

/// Convert a `std::future::Future` to a `tokio::Future`.
pub fn tokio_fut<T, F>(f: F) -> compat::backward::Compat<F>
where
    F: StdFuture<Output = Result<T>> + Send + 'static,
{
    compat::backward::Compat::new(f)
}

/// Wrap a future in a `Box`, so we can mix it with other futures that have the
/// same `Item` and `Error` type.
pub fn boxed_fut<T, F>(
    f: F,
) -> Box<dyn Future<Item = T, Error = failure::Error> + Send>
where
    F: StdFuture<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    Box::new(tokio_fut(f))
}
