use crate::{
    NetError, Result,
    codec::{Frames, MAX_BUFFER, OpcodeMap, Packet, Retained},
    profile::{ORIGIN, bridge},
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    time::{interval, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
use zeroize::Zeroizing;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const CLEANUP: Duration = Duration::from_millis(750);

pub(crate) struct Gateway {
    socket: Socket,
    frames: Frames,
    map: OpcodeMap,
    queued: VecDeque<Packet>,
    retained: Retained,
    token: Option<Zeroizing<String>>,
    received: u64,
    ack_sent: u64,
    opened: Instant,
    heard: Instant,
    last_ack: Instant,
    probe: Option<(Instant, u8)>,
}
impl Gateway {
    pub(crate) async fn open(host: &str, port: u16) -> Result<Self> {
        Self::open_url(bridge(host, port)?.as_str()).await
    }
    pub(crate) async fn open_url(url: &str) -> Result<Self> {
        let mut request = url.into_client_request().map_err(|_| NetError::Transport)?;
        request
            .headers_mut()
            .insert("Origin", ORIGIN.parse().map_err(|_| NetError::Transport)?);
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BUFFER))
            .max_frame_size(Some(MAX_BUFFER));
        let result = timeout(
            Duration::from_secs(15),
            connect_async_with_config(request, Some(config), false),
        )
        .await
        .map_err(|_| NetError::Timeout)?;
        let (socket, _) = result.map_err(ws_error)?;
        let now = Instant::now();
        Ok(Self {
            socket,
            frames: Frames::new(b"s2c"),
            map: OpcodeMap::current(b"c2s"),
            queued: VecDeque::new(),
            retained: Retained::default(),
            token: None,
            received: 0,
            ack_sent: 0,
            opened: now,
            heard: now,
            last_ack: now,
            probe: None,
        })
    }
    pub(crate) async fn send(
        &mut self,
        opcode: u16,
        payload: &[u8],
        order: &mut u64,
    ) -> Result<u64> {
        let bytes = self.map.encode(opcode, 0, payload)?;
        self.retained.sent(&bytes)?;
        *order = order
            .checked_add(1)
            .filter(|n| *n <= u64::MAX / 4)
            .ok_or(NetError::Capacity)?;
        let mark = *order;
        timeout(
            Duration::from_secs(2),
            self.socket.send(Message::Binary(bytes.to_vec().into())),
        )
        .await
        .map_err(|_| NetError::Uncertain)?
        .map_err(|_| NetError::Uncertain)?;
        Ok(mark)
    }
    pub(crate) fn complete_through(&self, order: u64) -> u64 {
        self.frames
            .pending_mark()
            .into_iter()
            .chain(self.queued.iter().map(|p| p.mark))
            .min()
            .map_or(order, |mark| mark.saturating_sub(1))
    }
    pub(crate) async fn receive(&mut self, order: &mut u64) -> Result<Packet> {
        if let Some(p) = self.queued.pop_front() {
            return Ok(p);
        }
        let mut ticks = interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                message=self.socket.next()=>{
                    let message=message.ok_or(NetError::Transport)?.map_err(ws_error)?;
                    match message {
                        Message::Binary(bytes)=>{
                            self.heard=Instant::now();self.probe=None;
                            self.received=self.received.checked_add(bytes.len() as u64).filter(|n|*n<=i64::MAX as u64).ok_or(NetError::Capacity)?;
                            self.queued.extend(self.frames.push(&bytes,order)?);
                            if let Some(p)=self.queued.pop_front() {return Ok(p);}
                        }
                        Message::Text(s)=>{
                            self.heard=Instant::now();self.probe=None;
                            match control(&s)? {
                                Control::Token(token)=>self.token=Some(token),
                                Control::Ack(n)=>self.retained.ack(n,false)?,
                                Control::Away(error)=>{self.token=None;return Err(error);}
                                Control::Version=>{},
                            }
                        }
                        Message::Ping(b)=>{self.socket.send(Message::Pong(b)).await.map_err(|_|NetError::Transport)?;}
                        Message::Pong(_)=>{},
                        Message::Close(frame)=>{
                            self.frames.finish()?;
                            if let Some(frame)=frame {
                                if frame.reason.to_ascii_lowercase().contains("gateway busy") || frame.reason.as_str()=="gatewayBusy" {return Err(NetError::Busy);}
                                if frame.code==tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy {return Err(NetError::Refused);}
                            }
                            return Err(NetError::Transport);
                        }
                        Message::Frame(_)=>return Err(NetError::Malformed),
                    }
                }
                _=ticks.tick()=>self.tick().await?,
            }
        }
    }
    async fn tick(&mut self) -> Result<()> {
        let now = Instant::now();
        if self.token.is_none() && now.duration_since(self.opened) >= Duration::from_secs(5) {
            self.retained.abandon();
        }
        if now.duration_since(self.heard) >= Duration::from_secs(5) {
            let (asked, ticks) = self.probe.get_or_insert((now, 0));
            *ticks = ticks.saturating_add(1);
            // Native sessions have an explicit always-visible foreground policy.
            if *ticks >= 2 && now.duration_since(*asked) >= Duration::from_secs(5) {
                return Err(NetError::Timeout);
            }
            self.socket
                .send(Message::Text("p".into()))
                .await
                .map_err(|_| NetError::Transport)?;
        }
        let delay = if self.received != self.ack_sent {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(10)
        };
        if now.duration_since(self.last_ack) >= delay {
            self.socket
                .send(Message::Text(format!("a {}", self.received).into()))
                .await
                .map_err(|_| NetError::Transport)?;
            self.ack_sent = self.received;
            self.last_ack = now;
        }
        Ok(())
    }
    /// Bounded local teardown. No server logout or immediate account availability is asserted.
    pub(crate) async fn close(mut self) {
        let token = self.token.take();
        let _ = timeout(Duration::from_millis(250), self.socket.close(None)).await;
        drop(self.socket);
        if let Some(token) = token
            && let Ok(client) = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(CLEANUP)
                .build()
        {
            let _ = timeout(
                CLEANUP,
                client
                    .post(format!("{ORIGIN}/net/release"))
                    .body(token.as_bytes().to_vec())
                    .send(),
            )
            .await;
        }
    }
}

fn ws_error(error: tokio_tungstenite::tungstenite::Error) -> NetError {
    use tokio_tungstenite::tungstenite::Error;
    match error {
        Error::Capacity(_) | Error::WriteBufferFull(_) => NetError::Capacity,
        Error::Protocol(_)
        | Error::Utf8(_)
        | Error::AttackAttempt
        | Error::Url(_)
        | Error::HttpFormat(_) => NetError::Malformed,
        Error::Http(response)
            if response.body().as_ref().is_some_and(|b| {
                b.len() <= 4096
                    && b.windows(12)
                        .any(|part| part.eq_ignore_ascii_case(b"gateway busy"))
            }) =>
        {
            NetError::Busy
        }
        error
            if error
                .to_string()
                .to_ascii_lowercase()
                .contains("gateway busy") =>
        {
            NetError::Busy
        }
        _ => NetError::Transport,
    }
}

enum Control {
    Token(Zeroizing<String>),
    Ack(u64),
    Away(NetError),
    Version,
}
fn control(s: &str) -> Result<Control> {
    if s.len() > 1024 || !s.is_ascii() {
        return Err(NetError::Malformed);
    }
    let parts: Vec<_> = s.split(' ').collect();
    match parts.as_slice() {
        ["k", token, seconds] => {
            if token.is_empty()
                || token.len() > 512
                || !token.bytes().all(|b| (0x21..=0x7e).contains(&b))
            {
                return Err(NetError::Malformed);
            }
            if seconds.is_empty() || !seconds.bytes().all(|b| b.is_ascii_digit()) {
                return Err(NetError::Malformed);
            }
            let seconds = seconds.parse::<f64>().map_err(|_| NetError::Malformed)?;
            if !seconds.is_finite() || seconds < 0.0 || seconds * 1000.0 > i64::MAX as f64 {
                return Err(NetError::Malformed);
            }
            Ok(Control::Token(Zeroizing::new((*token).to_owned())))
        }
        ["a", value] => {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(NetError::Malformed);
            }
            let n = value.parse::<u64>().map_err(|_| NetError::Malformed)?;
            if n > i64::MAX as u64 {
                return Err(NetError::Malformed);
            }
            Ok(Control::Ack(n))
        }
        ["g", "gatewayBusy"] => Ok(Control::Away(NetError::Busy)),
        ["g", "server"] => Ok(Control::Away(NetError::Transport)),
        ["g", _] => Ok(Control::Away(NetError::Refused)),
        ["v", build] if *build == crate::BUILD => Ok(Control::Version),
        ["v", _] => Err(NetError::BuildChanged),
        // No automatic resume or uncertain gameplay replay is implemented.
        _ => Err(NetError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_controls_fail_closed() {
        for s in [
            "a -1",
            "a +1",
            "a 9223372036854775808",
            "a 1 x",
            "k x NaN",
            "k x 1e2",
            "k x 1.5",
            "k  1",
            "r 0 token",
            "unknown",
            "v changed",
        ] {
            assert!(control(s).is_err());
        }
        assert!(matches!(control("a 7").unwrap(), Control::Ack(7)));
        assert!(matches!(
            control("k synthetic-token 1").unwrap(),
            Control::Token(_)
        ));
    }
    #[test]
    fn loopback_websocket_handshake_coalescing_ack_and_close() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
                let map = OpcodeMap::current(b"s2c");
                let mut bytes = map.encode(2014, 0, b"synthetic").unwrap();
                bytes.extend_from_slice(&map.encode(3, 0, b"").unwrap());
                ws.send(Message::Binary(bytes.to_vec().into()))
                    .await
                    .unwrap();
                let message = ws.next().await.unwrap().unwrap();
                assert!(matches!(message, Message::Binary(_)));
                ws.send(Message::Text("a 8".into())).await.unwrap();
                let _ = ws.next().await;
            });
            let mut client = Gateway::open_url(&format!("ws://{address}")).await.unwrap();
            let mut order = 0;
            assert_eq!(client.receive(&mut order).await.unwrap().opcode, 2014);
            assert_eq!(client.receive(&mut order).await.unwrap().opcode, 3);
            assert_eq!(client.send(29, &[1, 0], &mut order).await.unwrap(), 3);
            client.close().await;
            server.await.unwrap();
        });
    }
}
