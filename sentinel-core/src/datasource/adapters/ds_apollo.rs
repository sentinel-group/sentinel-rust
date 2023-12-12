use super::*;
use crate::{logging, utils::sleep_for_ms};
use apollo_client::conf::{requests::WatchRequest, ApolloConfClient};
use futures_util::{future, pin_mut, stream::StreamExt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct ApolloDatasource<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> {
    ds: DataSourceBase<P, H>,
    property: String,
    watch_request: WatchRequest,
    client: ApolloConfClient,
    closed: AtomicBool,
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> ApolloDatasource<P, H> {
    pub fn new(
        client: ApolloConfClient,
        property: String,
        watch_request: WatchRequest,
        handlers: Vec<Arc<H>>,
    ) -> Self {
        let mut ds = DataSourceBase::default();
        for h in handlers {
            ds.add_property_handler(h);
        }
        ApolloDatasource {
            ds,
            property,
            client,
            watch_request,
            closed: AtomicBool::new(false),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.watch().await
    }

    async fn watch(&mut self) -> Result<()> {
        logging::info!(
            "[Apollo] Apollo datasource is watching property {}",
            self.property
        );

        let stream = self
            .client
            .watch(self.watch_request.clone())
            .take_while(|_| future::ready(!self.closed.load(Ordering::SeqCst)));

        pin_mut!(stream);

        while let Some(response) = stream.next().await {
            match response {
                Ok(responses) => {
                    // Load rules
                    // One namespace for one response
                    for (_, value) in responses {
                        match value {
                            Ok(r) => {
                                let rule = r.configurations.get(&self.property);
                                if let Err(e) = self.ds.update(rule) {
                                    logging::error!("[Apollo] Failed to update rules, {:?}", e);
                                }
                            }
                            Err(e) => logging::error!(
                                "[Apollo] Fail to fetch response from apollo, {:?}",
                                e
                            ),
                        };
                    }
                }
                // retry
                Err(e) => {
                    logging::error!("[Apollo] Client yield an error, {:?}", e);
                    sleep_for_ms(1000);
                }
            }
        }

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        logging::info!(
            "[Apollo] Apollo data source has been closed. Stop watch the key {:?} from apollo.",
            self.property
        );
        Ok(())
    }
}

impl<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> DataSource<P, H>
    for ApolloDatasource<P, H>
{
    fn get_base(&mut self) -> &mut DataSourceBase<P, H> {
        &mut self.ds
    }
}
