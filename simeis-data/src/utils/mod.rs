mod access_api;
mod macros;
mod shardeddata;
use std::future::Future;
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub use shardeddata::{ShardedLockedData, PLAYER_SHARDS};
pub fn timestamp() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
