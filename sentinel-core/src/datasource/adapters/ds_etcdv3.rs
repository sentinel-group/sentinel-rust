use super::*;
use crate::{logging, utils::sleep_for_ms};
use etcd_rs::{Client, DeleteRequest, EventType, KeyRange, RangeRequest, WatchResponse};
use futures::StreamExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Etcdv3DataSource<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> {
    ds: DataSourceBase<P, H>,
    property: String,
    last_updated_revision: u64,
    client: Client,
    closed: AtomicBool,
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> Etcdv3DataSource<P, H> {
    pub fn new(client: Client, property: String, handlers: Vec<Arc<H>>) -> Self {
        let mut ds = DataSourceBase::default();
        for h in handlers {
            // incase of duplication, add it one by one, instead of adding all at once
            ds.add_property_handler(h);
        }
        Etcdv3DataSource {
            ds,
            property,
            last_updated_revision: 0,
            client,
            closed: AtomicBool::new(false),
        }
    }

    /// initialize the datasource and load initial rules
    /// start listener to listen on dynamic source
    /// return error if initialize failed.
    pub async fn initialize(&mut self) -> Result<()> {
        self.read_and_update().await?;
        self.watch().await
    }

    /// Close the data source, stop watch the property key.
    pub async fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        self.client
            .kv()
            .delete(DeleteRequest::new(KeyRange::key(&self.property[..])))
            .await?;
        self.client.shutdown().await?;
        logging::info!(
            "[Etcd-v3] Etcd-v3 data source has been closed. Remove the key {:?} from Etcd-v3.",
            self.property
        );
        Ok(())
    }

    async fn read_and_update(&mut self) -> Result<()> {
        let src = self.read_source().await?;
        if src.is_empty() {
            self.get_base().update(None).unwrap();
        } else {
            self.get_base().update(Some(&src)).unwrap();
        }
        Ok(())
    }

    /// Read original data from the data source.
    /// return source bytes if succeed to read, if not, return error when reading
    async fn read_source(&mut self) -> Result<String> {
        let mut resp = self
            .client
            .kv()
            .range(RangeRequest::new(KeyRange::key(&self.property[..])))
            .await?;
        let kvs = resp.take_kvs();
        if kvs.is_empty() {
            return Err(Error::msg(format!(
                "The key {} is not existed in the etcd server.",
                self.property
            )));
        }
        let header = resp.take_header().ok_or_else(|| {
            Error::msg(format!(
                "The header of  key {} is not existed in the etcd server",
                self.property
            ))
        })?;
        self.last_updated_revision = header.revision();
        logging::info!(
            "[Etcdv3] Get the newest data for key {}, with revision {} and value {}",
            self.property,
            header.revision(),
            kvs[0].value_str()
        );
        Ok(kvs[0].value_str().to_owned())
    }

    /// Add watch for property from last_updated_revision updated after initializing
    async fn watch(&mut self) -> Result<()> {
        logging::info!(
            "[Etcd] Etcd-v3 data source is watching property {:?}",
            self.property
        );
        loop {
            let mut inbound = self.client.watch(KeyRange::key(&self.property[..])).await?;
            while let Some(resp) = inbound.next().await {
                let resp = resp?.ok_or_else(|| {
                    Error::msg(format!(
                        "Watch a None response for key {} in the etcd server",
                        self.property
                    ))
                })?;
                self.process_watch_response(resp).await?;
            }
            if self.closed.load(Ordering::SeqCst) {
                return Ok(());
            }
            sleep_for_ms(1000);
        }
    }

    async fn process_watch_response(&mut self, mut resp: WatchResponse) -> Result<()> {
        let header = resp.take_header().ok_or_else(|| {
            Error::msg(format!(
                "The header of  key {} is not existed in the etcd server",
                self.property
            ))
        })?;
        if header.revision() > self.last_updated_revision {
            self.last_updated_revision = header.revision();
            for ev in resp.take_events() {
                match ev.event_type() {
                    EventType::Put => {
                        if (self.read_and_update().await).is_err() {
                            logging::error!(
                                "Fail to execute process_watch_response() for PUT event"
                            );
                        }
                    }
                    EventType::Delete => {
                        if self.ds.update(None).is_err() {
                            logging::error!(
                                "Fail to execute process_watch_response() for DELETE event"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> DataSource<P, H>
    for Etcdv3DataSource<P, H>
{
    fn get_base(&mut self) -> &mut DataSourceBase<P, H> {
        &mut self.ds
    }
}
