use defmt::*;
use embassy_net::tcp::TcpSocket;
use embassy_time::Duration;
use embedded_io_async::Write;
use embassy_sync::pubsub::Subscriber;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use serde::Serialize;
use embassy_net::Stack;

#[derive(Serialize)]
struct Message {
    temperature: Option<f32>,
}

pub async fn listen(
    stack: Stack<'_>,
    subscriber: &mut Subscriber<'_, CriticalSectionRawMutex, Option<f32>, 4, 4, 4>
) {
    let mut rx_buffer = [0; 8192];
    let mut tx_buffer = [0; 8192];

    loop {
        println!("before socket");
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));
        println!("before accept");
        if let Err(e) = socket.accept(1234).await {
            println!("could not open socket");
            continue;
        }
        info!("Received connection from {:?}", socket.remote_endpoint());

        let mut deserialized = [0u8; 512];
        let measurement = Message {
            temperature: subscriber.next_message_pure().await
        };
        let len = serde_json_core::to_slice(&measurement, &mut deserialized[..]).unwrap();
        if let Err(e) = socket.write_all(&deserialized[..len]).await {
            println!("could not write");
            continue;
        }
        if let Err(e) = socket.flush().await {
            println!("could not flush");
            continue;
        }
        socket.abort();
    }
}
