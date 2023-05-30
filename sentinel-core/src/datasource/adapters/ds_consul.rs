use super::*;
use crate::{logging, utils::sleep_for_ms, Error};
use consul::{kv::KV, Client, QueryOptions};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct ConsulDataSource<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> {
    ds: DataSourceBase<P, H>,
    query_options: QueryOptions,
    property: String,
    client: Client,
    closed: AtomicBool,
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> ConsulDataSource<P, H> {
    pub fn new(
        client: Client,
        query_options: QueryOptions,
        property: String,
        handlers: Vec<Arc<H>>,
    ) -> Self {
        let mut ds = DataSourceBase::default();
        for h in handlers {
            // incase of duplication, add it one by one, instead of adding all at once
            ds.add_property_handler(h);
        }
        ConsulDataSource {
            ds,
            query_options,
            property,
            client,
            closed: AtomicBool::new(false),
        }
    }

    /// initialize the datasource and load initial rules
    /// start listener to listen on dynamic source
    /// return error if initialize failed.
    pub fn initialize(&mut self) -> Result<()> {
        self.read_and_update()?;
        self.watch()
    }

    /// Close the data source, stop watch the property key.
    pub fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        self.client.delete(&self.property[..], None).unwrap();
        logging::info!(
            "[Consul] Consul data source has been closed. Remove the key {:?} from Consul.",
            self.property
        );
        Ok(())
    }

    fn read_and_update(&mut self) -> Result<()> {
        let src = self.read_source()?;
        if src.is_empty() {
            self.get_base().update(None).unwrap();
        } else {
            self.get_base().update(Some(&src)).unwrap();
        }
        Ok(())
    }

    /// Read original data from the data source.
    /// return source bytes if succeed to read, if not, return error when reading
    fn read_source(&mut self) -> Result<String> {
        let (kv, meta) = self
            .client
            .get(&self.property[..], Some(&self.query_options))
            .unwrap();
        let kv = kv.ok_or_else(|| {
            Error::msg(format!("[Consul] Cannot find the key {:?}.", self.property))
        })?;
        self.query_options.wait_index = meta.last_index;
        let mut bytes = base64::decode(kv.Value).unwrap();
        bytes.remove(bytes.len() - 1);
        bytes.remove(0);
        let value = String::from_utf8(bytes).unwrap();
        let value = value.replace('\\', "");
        Ok(value)
    }

    /// Add watch for property from last_updated_revision updated after initializing
    fn watch(&mut self) -> Result<()> {
        logging::info!(
            "[Consul] Consul data source is watching property {:?}",
            self.property
        );
        loop {
            self.read_and_update()?;
            if self.closed.load(Ordering::SeqCst) {
                return Ok(());
            }
            sleep_for_ms(1000);
        }
    }
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> DataSource<P, H>
    for ConsulDataSource<P, H>
{
    fn get_base(&mut self) -> &mut DataSourceBase<P, H> {
        &mut self.ds
    }
}
