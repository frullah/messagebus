use async_trait::async_trait;
use messagebus::{
    derive::{Error as MbError, Message},
    error, receivers, AsyncHandler, Bus, Message,
};
use thiserror::Error;

#[derive(Debug, Error, MbError)]
enum Error {
    #[error("Error({0})")]
    Error(anyhow::Error),
}

impl<M: Message> From<error::Error<M>> for Error {
    fn from(err: error::Error<M>) -> Self {
        Self::Error(err.into())
    }
}

#[derive(Debug, Clone, Message)]
struct MsgF32(pub f32);

struct TmpReceiver;

#[async_trait]
impl AsyncHandler<MsgF32> for TmpReceiver {
    type Error = Error;
    type Response = ();

    async fn handle(&self, msg: MsgF32, _bus: &Bus) -> Result<Self::Response, Self::Error> {
        println!("---> f32 {:?}", msg);

        std::thread::sleep(std::time::Duration::from_secs(1));

        println!("done");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let (b, poller) = Bus::build()
        .register(TmpReceiver)
        .subscribe_async::<MsgF32>(
            1,
            receivers::BufferUnorderedConfig {
                buffer_size: 1,
                max_parallel: 1,
            },
        )
        .done()
        .build();

    println!("sending 1");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 2");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 3");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 4");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 5");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 6");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 7");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 8");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 9");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 10");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("sending 11");
    b.send(MsgF32(32f32)).await.unwrap();

    println!("flush");
    b.flush().await;

    println!("close");
    b.close().await;

    println!("closed");

    poller.await;
    println!("[done]");
}
