use anyhow::Result;
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

pub struct FutuClient {
    write_sink: WriteSink,
    serial_no: Arc<AtomicU32>,
    pending_requests: PendingRequests,
}

impl FutuClient {
    pub fn conn_id(&self) -> u64 {
        0 // Currently OpenD connect doesn't expose conn_id explicitly in InitConnect response in this implementation yet, 0 is safe bypass.
    }

    pub fn next_serial(&self) -> u32 {
        self.serial_no.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn connect(addr: &str) -> Result<Self> {
        println!("🔌 Connecting to Moomoo OpenD at {}", addr);
        let stream = TcpStream::connect(addr).await?;
        let framed = Framed::new(stream, FutuCodec);

        let (mut sink, mut stream_reader) = framed.split();
        let serial_no = Arc::new(AtomicU32::new(1));

        let pending_requests: PendingRequests =
            Arc::new(Mutex::new(HashMap::new()));

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
                        if parsed.ret_type == 0 {
                            let s2c = parsed.s2c.unwrap();
                            keep_alive_interval = s2c.keep_alive_interval;
                            println!(
                                "✅ Successfully authenticated with Moomoo OpenD! Server Ver: {}",
                                s2c.server_ver
                            );
                        } else {
                            anyhow::bail!("InitConnect failed: {:?}", parsed.ret_msg);
                        }
                    } else {
                        anyhow::bail!(
                            "Expected InitConnect Response (1001) but got {}",
                            res_header.n_proto_id
                        );
                    }
                }
                Err(e) => anyhow::bail!("Codec read error during auth: {}", e),
            }
        } else {
            anyhow::bail!("Connection closed during InitConnect");
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

        // Background loop to multiplex responses
        tokio::spawn(async move {
            while let Some(res) = stream_reader.next().await {
                match res {
                    Ok((res_header, res_body)) => {
                        let serial = res_header.n_serial_no;
                        let mut pending = pending_clone.lock().await;
                        if let Some(sender) = pending.remove(&serial) {
                            let _ = sender.send((res_header, res_body));
                        } else if res_header.n_proto_id == 1004 { // KeepAlive Response
                             // Silently drop keepalive ACKs to avoid log spam
                        } else {
                            println!(
                                "📩 Unmatched or Push Notification received! ProtoID: {}",
                                res_header.n_proto_id
                            );
                        }
                    }
                    Err(e) => {
                        println!("❌ FutuClient background loop error: {}", e);
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
                        "Protocol ID mismatch! Expected {} got {}",
                        proto_id,
                        res_header.n_proto_id
                    )
                }
            }
            Ok(Err(_)) => anyhow::bail!("Request cancelled or channel closed"),
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&serial);
                anyhow::bail!("Request timed out waiting for OpenD response")
            }
        }
    }
}
