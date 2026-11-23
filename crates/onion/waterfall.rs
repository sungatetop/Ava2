//! 洋葱瀑布组合子：listener 自外向内包裹终端（内核默认行为）。
//!
//! Koa 式 `next`：listener 拿到 `Next<In, Out>`，调用 `next(input)` 即委托内层
//! （可传入改写后的 input）；不调用直接返回 = 短路（内层与终端均不执行）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::listener::{Listener, ListenerCtx};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// 下游处理器：`next(改写后的输入)` 返回内层 future。
pub type Next<In, Out> = Box<dyn FnOnce(In) -> BoxFuture<Out> + Send>;

/// 单层调用器：由具体边界（listener.rs 的 trait 方法）提供。
pub type Invoke<Req, Out> =
    dyn Fn(Arc<dyn Listener>, ListenerCtx, Req, Next<Req, Out>) -> BoxFuture<Out> + Send + Sync;

/// 自外向内折叠：listeners[0] 最外层，terminal 是内核默认行为（最内层）。
/// 构造次序：先 terminal，再逆序包 listener——保证外层先入后出（LIFO）。
pub fn fold<Req, Out>(
    listeners: &[Arc<dyn Listener>],
    ctx: ListenerCtx,
    req: Req,
    terminal: impl FnOnce(Req) -> BoxFuture<Out> + Send + 'static,
    invoke: impl Fn(Arc<dyn Listener>, ListenerCtx, Req, Next<Req, Out>) -> BoxFuture<Out>
        + Copy
        + Send
        + Sync
        + 'static,
) -> BoxFuture<Out>
where
    Req: Clone + Send + 'static,
    Out: Send + 'static,
{
    let mut chain: Box<dyn FnOnce(Req) -> BoxFuture<Out> + Send> =
        Box::new(move |input| terminal(input));
    for listener in listeners.iter().rev() {
        let listener = listener.clone();
        let inner = chain;
        let ctx = ctx.clone();
        chain = Box::new(move |input| {
            let listener = listener.clone();
            let ctx = ctx.clone();
            let next: Next<Req, Out> = Box::new(inner);
            Box::pin(async move { invoke(listener, ctx, input, next).await })
        });
    }
    chain(req)
}

#[cfg(test)]
#[path = "waterfall_tests.rs"]
mod tests;
