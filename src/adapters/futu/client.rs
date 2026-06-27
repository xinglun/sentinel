use anyhow::{Context, Result};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use prost::Message;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_util::codec::Framed;

use crate::adapters::futu::codec::{FutuCodec, FutuHeader};
use crate::adapters::futu::protocol::generated::init_connect::{C2s, Request, Response};

pub type PendingRequests = Arc<Mutex<HashMap<u32, oneshot::Sender<(FutuHeader, Vec<u8>)>>>>;
pub type WriteSink = Arc<Mutex<SplitSink<Framed<TcpStream, FutuCodec>, (FutuHeader, Vec<u8>)>>>;

#[derive(Debug)]
struct InitConnectAck {
    server_ver: i32,
    keep_alive_interval: i32,
}

pub struct FutuClient {
    write_sink: WriteSink,
    serial_no: Arc<AtomicU32>,
    pending_requests: PendingRequests,
}

fn connect_notice(addr: &str) -> String {
    format!("🔌 Moomoo OpenD へ接続します: {}", addr)
}

impl FutuClient {
    pub fn conn_id(&self) -> u64 {
        0 // 現状の実装では InitConnect 応答から conn_id を明示的に取得しないため、0 を安全な代替値として使う。
    }

    pub fn next_serial(&self) -> u32 {
        self.serial_no.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn connect(addr: &str) -> Result<Self> {
        println!("{}", connect_notice(addr));
        let stream = TcpStream::connect(addr).await?;
        let framed = Framed::new(stream, FutuCodec);

        let (mut sink, mut stream_reader) = framed.split();
        let serial_no = Arc::new(AtomicU32::new(1));

        let pending_requests: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        let init_req = Request {
            c2s: C2s {
                client_ver: 300,
                client_id: "sentinel_rs_v1".to_string(),
                recv_notify: Some(true),
                packet_enc_algo: Some(0),
                push_proto_fmt: Some(0),
                programming_language: Some("Rust".to_string()),
            },
        };

        let mut body = Vec::new();
        init_req.encode(&mut body)?;
        let req_serial = serial_no.fetch_add(1, Ordering::SeqCst);
        let header = FutuHeader::new(1001, req_serial, body.len() as u32);

        sink.send((header, body)).await?;

        #[allow(unused_assignments)]
        let mut keep_alive_interval = 10;

        if let Some(res) = stream_reader.next().await {
            match res {
                Ok((res_header, res_body)) => {
                    if res_header.n_proto_id == 1001 {
                        let parsed = Response::decode(&res_body[..])?;
                        let ack = parse_init_connect_ack(parsed)?;
                        keep_alive_interval = ack.keep_alive_interval;
                        println!(
                            "✅ Moomoo OpenD への認証に成功しました。Server Ver: {}",
                            ack.server_ver
                        );
                    } else {
                        anyhow::bail!(
                            "InitConnect 応答 (1001) を期待しましたが {} が返りました",
                            res_header.n_proto_id
                        );
                    }
                }
                Err(e) => anyhow::bail!("認証中の codec 読み取りに失敗しました: {}", e),
            }
        } else {
            anyhow::bail!("InitConnect 中に接続が閉じられました");
        }

        let pending_clone = Arc::clone(&pending_requests);
        let write_sink = Arc::new(Mutex::new(sink));

        let hb_sink = Arc::clone(&write_sink);
        let hb_serial = Arc::clone(&serial_no);

        if keep_alive_interval > 0 {
            tokio::spawn(async move {
                use crate::adapters::futu::protocol::generated::keep_alive::{
                    C2s as HbC2s, Request as HbRequest,
                };
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                    keep_alive_interval as u64,
                ));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    let req = HbRequest {
                        c2s: HbC2s {
                            time: chrono::Utc::now().timestamp(),
                        },
                    };
                    let mut body = Vec::new();
                    if req.encode(&mut body).is_ok() {
                        let serial = hb_serial.fetch_add(1, Ordering::SeqCst);
                        let header = FutuHeader::new(1004, serial, body.len() as u32);
                        let mut sink = hb_sink.lock().await;
                        let _ = sink.send((header, body)).await;
                    }
                }
            });
        }

        // 応答を多重化する background loop。
        tokio::spawn(async move {
            while let Some(res) = stream_reader.next().await {
                match res {
                    Ok((res_header, res_body)) => {
                        let serial = res_header.n_serial_no;
                        let mut pending = pending_clone.lock().await;
                        if let Some(sender) = pending.remove(&serial) {
                            let _ = sender.send((res_header, res_body));
                        } else if res_header.n_proto_id == 1004 {
                            // ログ spam を避けるため、keepalive ACK は静かに破棄する。
                        } else {
                            println!(
                                "📩 一致しない応答または Push 通知を受信しました。ProtoID: {}",
                                res_header.n_proto_id
                            );
                        }
                    }
                    Err(e) => {
                        println!(
                            "❌ FutuClient の background loop でエラーが発生しました: {}",
                            e
                        );
                        break;
                    }
                }
            }
        });

        Ok(Self {
            write_sink,
            serial_no,
            pending_requests,
        })
    }

    pub async fn send_request<M: Message>(&self, proto_id: u32, msg: &M) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        msg.encode(&mut body)?;

        let serial = self.serial_no.fetch_add(1, Ordering::SeqCst);
        let header = FutuHeader::new(proto_id, serial, body.len() as u32);

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(serial, tx);
        }

        {
            let mut sink = self.write_sink.lock().await;
            sink.send((header, body)).await?;
        }

        match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
            Ok(Ok((res_header, res_body))) => {
                if res_header.n_proto_id == proto_id {
                    Ok(res_body)
                } else {
                    anyhow::bail!(
                        "Protocol ID が一致しません。期待値 {} / 実際 {}",
                        proto_id,
                        res_header.n_proto_id
                    )
                }
            }
            Ok(Err(_)) => anyhow::bail!("リクエストがキャンセルされたか、channel が閉じられました"),
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&serial);
                anyhow::bail!("OpenD 応答待ちがタイムアウトしました")
            }
        }
    }
}

fn parse_init_connect_ack(parsed: Response) -> Result<InitConnectAck> {
    if parsed.ret_type != 0 {
        anyhow::bail!("InitConnect に失敗しました: {:?}", parsed.ret_msg);
    }

    let s2c = parsed
        .s2c
        .context("InitConnect は成功しましたが応答の s2c がありません")?;

    Ok(InitConnectAck {
        server_ver: s2c.server_ver,
        keep_alive_interval: s2c.keep_alive_interval,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::futu::codec::{FutuCodec, FutuHeader};
    use crate::adapters::futu::protocol::generated::init_connect::S2c;
    use futures::{SinkExt, StreamExt};
    use prost::Message;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_util::codec::Framed;

    #[test]
    fn parse_init_connect_ack_rejects_missing_s2c() {
        let response = Response {
            ret_type: 0,
            ret_msg: None,
            err_code: None,
            s2c: None,
        };

        let err = parse_init_connect_ack(response).unwrap_err();

        assert!(err.to_string().contains("s2c"), "unexpected error: {}", err);
    }

    #[test]
    fn parse_init_connect_ack_extracts_keep_alive_interval() {
        let response = Response {
            ret_type: 0,
            ret_msg: None,
            err_code: None,
            s2c: Some(S2c {
                server_ver: 123,
                login_user_id: 456,
                conn_id: 789,
                conn_aes_key: "key1234567890123".to_string(),
                keep_alive_interval: 30,
                aes_cb_civ: None,
                user_attribution: None,
            }),
        };

        let ack = parse_init_connect_ack(response).unwrap();

        assert_eq!(ack.server_ver, 123);
        assert_eq!(ack.keep_alive_interval, 30);
    }

    #[tokio::test]
    async fn connect_and_send_request_round_trip_via_local_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut framed = Framed::new(socket, FutuCodec);

            let (init_header, init_body) = framed.next().await.unwrap().unwrap();
            assert_eq!(init_header.n_proto_id, 1001);
            assert!(!init_body.is_empty());

            let mut ack_body = Vec::new();
            Response {
                ret_type: 0,
                ret_msg: None,
                err_code: None,
                s2c: Some(S2c {
                    server_ver: 456,
                    login_user_id: 123,
                    conn_id: 789,
                    conn_aes_key: "1234567890abcdef".to_string(),
                    keep_alive_interval: 30,
                    aes_cb_civ: None,
                    user_attribution: None,
                }),
            }
            .encode(&mut ack_body)
            .unwrap();

            framed
                .send((
                    FutuHeader::new(1001, init_header.n_serial_no, ack_body.len() as u32),
                    ack_body,
                ))
                .await
                .unwrap();

            let (request_header, request_body) = framed.next().await.unwrap().unwrap();
            assert_eq!(request_header.n_proto_id, 3201);
            assert!(!request_body.is_empty());

            framed
                .send((
                    FutuHeader::new(3201, request_header.n_serial_no, 0),
                    Vec::new(),
                ))
                .await
                .unwrap();
        });

        let client = timeout(
            Duration::from_secs(5),
            FutuClient::connect(&addr.to_string()),
        )
        .await
        .unwrap()
        .unwrap();

        let request = Request {
            c2s: C2s {
                client_ver: 300,
                client_id: "sentinel_rs_v1".to_string(),
                recv_notify: Some(true),
                packet_enc_algo: Some(0),
                push_proto_fmt: Some(0),
                programming_language: Some("Rust".to_string()),
            },
        };

        let raw = timeout(Duration::from_secs(5), client.send_request(3201, &request))
            .await
            .unwrap()
            .unwrap();

        assert!(raw.is_empty());

        drop(client);
        server.await.unwrap();
    }

    #[test]
    fn connect_notice_is_localized() {
        assert!(connect_notice("127.0.0.1:11111").contains("接続"));
    }
}
